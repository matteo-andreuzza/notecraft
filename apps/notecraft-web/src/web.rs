//! The browser shell: the clock, the leave-page guard and the eframe web runner.

use std::cell::Cell;
use std::rc::Rc;

use notecraft_engine::{Clock, Session};
use notecraft_ui_egui::{Host, NoteApp};
use wasm_bindgen::JsCast as _;

const CANVAS_ID: &str = "notecraft_canvas";
const LOADING_ID: &str = "notecraft_loading";

/// The browser's clock (through chrono's `wasmbind`: `Date`). No dialogs, no backups.
struct WebHost;

impl Host for WebHost {
    fn clock(&self) -> Clock {
        let now = chrono::Local::now();
        Clock { now_ms: now.timestamp_millis(), utc_offset_min: now.offset().local_minus_utc() / 60 }
    }
}

/// The app, and the flag the leave-page guard reads.
struct WebShell {
    app: NoteApp,
    start_revision: u64,
    changed: Rc<Cell<bool>>,
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.logic(ctx);
        // Notebooks live in memory here: anything changed is lost with the tab.
        self.changed.set(self.app.session.revision != self.start_revision);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// Ask before leaving the page once something changed (the notes would be lost).
fn guard_unload(changed: Rc<Cell<bool>>) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let on_unload = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(move |e: web_sys::BeforeUnloadEvent| {
        if changed.get() {
            e.prevent_default();
            // Older browsers ask only when `returnValue` is set.
            e.set_return_value("unsaved changes");
        }
    });
    window.add_event_listener_with_callback("beforeunload", on_unload.as_ref().unchecked_ref()).map_err(|e| format!("{e:?}"))?;
    // The listener lives as long as the page.
    on_unload.forget();
    Ok(())
}

/// Start the app on the page's canvas.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let changed = Rc::new(Cell::new(false));
        if let Err(e) = guard_unload(changed.clone()) {
            log::error!("no leave-page guard: {e}");
        }
        let mut options = eframe::WebOptions::default();
        if query().contains("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    if let Some(rs) = &cc.wgpu_render_state {
                        log::info!("notecraft-web: wgpu backend {:?}", rs.adapter.get_info().backend);
                    }
                    let host = WebHost;
                    let app = NoteApp::new(Session::with_sample(host.clock()), Box::new(host));
                    let start_revision = app.session.revision;
                    Ok(Box::new(WebShell { app, start_revision, changed }))
                }),
            )
            .await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>NoteCraft failed to start: {e:?}</p><p>A browser with WebGPU or WebGL2 is required.</p>")),
            }
        }
    });
}
