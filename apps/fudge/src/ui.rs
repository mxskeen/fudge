//! The launcher window (GTK4 + libadwaita).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use fudge_core::{Engine, Icon, Platform, ResultItem, Style};
use gtk::{gdk, glib};
use gtk4 as gtk;
use libadwaita as adw;

use crate::ipc::Cmd;
use crate::keys::{self, EscOutcome, KeyAction};

const CSS: &str = include_str!("style.css");
const WIDTH: i32 = 640;
const DEFAULT_FOOTER: &str = "↑↓ move · Enter run · Ctrl+Enter alt · Esc clear";

thread_local! {
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

fn with_ui(f: impl FnOnce(&Rc<Ui>)) {
    let ui = UI.with(|u| u.borrow().clone());
    if let Some(ui) = ui {
        f(&ui);
    } else {
        log::warn!("command arrived before the window was built; dropped");
    }
}

/// Thread-safe entry point: queue `cmd` on the GTK main loop. `time` is the X11 event timestamp or 0.
pub fn dispatch(cmd: Cmd, time: u32) {
    let at = Instant::now();
    glib::MainContext::default().invoke(move || with_ui(|ui| ui.handle(cmd, time, at)));
}

pub fn debug_timing() -> bool {
    std::env::var("FUDGE_DEBUG_TIMING").map(|v| v == "1").unwrap_or(false)
}

struct Ui {
    engine: Arc<Engine>,
    platform: Arc<dyn Platform>,
    window: adw::ApplicationWindow,
    entry: gtk::Entry,
    list: gtk::ListBox,
    list_wrap: gtk::Box,
    footer: gtk::Label,
    items: RefCell<Vec<ResultItem>>,
    hints: RefCell<Vec<gtk::Box>>,
    selected: Cell<usize>,
    gen: Cell<u64>,
    confirm: Cell<Option<(usize, usize)>>,
    polling: Cell<bool>,
    last_epoch: Cell<u64>,
    toast_gen: Cell<u64>,
    cmd_at: Cell<Option<Instant>>,
    #[cfg(target_os = "linux")]
    x11: RefCell<Option<crate::placement::Placer>>,
}

pub fn build(app: &adw::Application, engine: Arc<Engine>, platform: Arc<dyn Platform>) {
    let display = gdk::Display::default().expect("no display");
    let provider = gtk::CssProvider::new();
    #[allow(deprecated)]
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    let entry = gtk::Entry::builder()
        .placeholder_text("Search apps, files, math…")
        .hexpand(true)
        .build();
    entry.add_css_class("fudge-entry");

    let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).build();
    list.add_css_class("fudge-list");
    list.set_activate_on_single_click(true);
    let list_wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list_wrap.append(&list);

    let footer = gtk::Label::builder().label(DEFAULT_FOOTER).xalign(0.0).build();
    footer.add_css_class("fudge-footer");

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.add_css_class("fudge-panel");
    panel.set_overflow(gtk::Overflow::Hidden);
    panel.append(&entry);
    panel.append(&list_wrap);
    panel.append(&footer);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Fudge Launcher")
        .decorated(false)
        .resizable(false)
        .default_width(WIDTH)
        .content(&panel)
        .build();
    window.add_css_class("fudge-window");
    if engine.config().style == Style::Frosted {
        // Best effort: only translucent when a compositing window manager is running.
        window.add_css_class("frosted");
    }
    window.set_visible(false);

    let ui = Rc::new(Ui {
        engine,
        platform,
        window: window.clone(),
        entry: entry.clone(),
        list: list.clone(),
        list_wrap,
        footer,
        items: RefCell::new(Vec::new()),
        hints: RefCell::new(Vec::new()),
        selected: Cell::new(0),
        gen: Cell::new(0),
        confirm: Cell::new(None),
        polling: Cell::new(false),
        last_epoch: Cell::new(0),
        toast_gen: Cell::new(0),
        cmd_at: Cell::new(None),
        #[cfg(target_os = "linux")]
        x11: RefCell::new(None),
    });

    // Colour scheme: libadwaita decides, we toggle a class.
    let sm = adw::StyleManager::default();
    let sync_scheme = {
        let w = window.clone();
        move |sm: &adw::StyleManager| {
            if sm.is_dark() {
                w.remove_css_class("light");
            } else {
                w.add_css_class("light");
            }
        }
    };
    sync_scheme(&sm);
    sm.connect_dark_notify(sync_scheme);

    // Typing.
    {
        let ui = ui.clone();
        entry.connect_changed(move |_| {
            ui.confirm.set(None);
            ui.reset_footer();
            ui.request_search();
        });
    }
    // Clicking a row.
    {
        let ui = ui.clone();
        list.connect_row_activated(move |_, row| {
            ui.run_index(row.index() as usize, 0);
        });
    }
    // Keys (capture so the entry never swallows navigation).
    {
        let ui = ui.clone();
        let kc = gtk::EventControllerKey::new();
        kc.set_propagation_phase(gtk::PropagationPhase::Capture);
        kc.connect_key_pressed(move |_, key, _, state| {
            match keys::map_key(key, state) {
                Some(a) => {
                    ui.on_key(a);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        window.add_controller(kc);
    }
    // Hide on focus loss.
    {
        let ui = ui.clone();
        window.connect_is_active_notify(move |w| {
            if !w.is_active() && w.is_visible() {
                ui.hide();
            }
        });
    }
    window.connect_close_request(|w| {
        w.set_visible(false);
        glib::Propagation::Stop
    });
    // Timing: command received -> mapped (and first frame).
    {
        let ui = ui.clone();
        window.connect_map(move |w| {
            #[cfg(target_os = "linux")]
            ui.place_window();
            if debug_timing() {
                if let Some(t) = ui.cmd_at.take() {
                    eprintln!("[timing] command -> map: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
                    w.add_tick_callback(move |_, _| {
                        eprintln!("[timing] command -> first frame: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
                        glib::ControlFlow::Break
                    });
                }
            }
            let _ = &ui;
        });
    }

    #[cfg(feature = "layer-shell")]
    ui.setup_layer_shell();

    UI.with(|u| *u.borrow_mut() = Some(ui.clone()));
    ui.last_epoch.set(ui.engine.epoch_value());
    // Pre-realize so the first show is as fast as later ones.
    WidgetExt::realize(&window);
    ui.apply_results(ui.gen.get(), Vec::new());
}

impl Ui {
    fn limit(&self) -> usize {
        self.engine.config().max_results.clamp(1, 12)
    }

    fn handle(self: &Rc<Self>, cmd: Cmd, time: u32, at: Instant) {
        match cmd {
            Cmd::Show => self.show(time, at),
            Cmd::Hide => self.hide(),
            Cmd::Toggle => {
                if self.window.is_visible() {
                    self.hide();
                } else {
                    self.show(time, at);
                }
            }
            Cmd::Reindex => self.engine.refresh_all(),
        }
    }

    fn show(self: &Rc<Self>, time: u32, at: Instant) {
        if self.window.is_visible() {
            self.window.present();
            self.entry.grab_focus();
            return;
        }
        self.cmd_at.set(Some(at));
        self.confirm.set(None);
        self.reset_footer();
        self.entry.set_text("");
        self.gen.set(self.gen.get() + 1);
        let recents = self.engine.search("", self.limit());
        self.apply_results(self.gen.get(), recents);
        self.last_epoch.set(self.engine.epoch_value());
        #[cfg(target_os = "linux")]
        self.place_window();
        if time != 0 {
            self.window.present_with_time(time);
        } else {
            self.window.present();
        }
        self.entry.grab_focus();
        self.start_polling();
    }

    fn hide(&self) {
        self.window.set_visible(false);
        self.confirm.set(None);
    }

    fn start_polling(self: &Rc<Self>) {
        if self.polling.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::timeout_add_local(Duration::from_millis(150), move || {
            if !ui.window.is_visible() {
                ui.polling.set(false);
                return glib::ControlFlow::Break;
            }
            let e = ui.engine.epoch_value();
            if e != ui.last_epoch.get() {
                ui.last_epoch.set(e);
                ui.request_search();
            }
            glib::ControlFlow::Continue
        });
    }

    fn request_search(self: &Rc<Self>) {
        let q = self.entry.text().to_string();
        let g = self.gen.get() + 1;
        self.gen.set(g);
        let engine = self.engine.clone();
        let limit = self.limit();
        let spawned = std::thread::Builder::new().name("fudge-search".into()).spawn(move || {
            let r = engine.search(&q, limit);
            glib::MainContext::default().invoke(move || with_ui(|ui| ui.apply_results(g, r)));
        });
        if let Err(e) = spawned {
            log::error!("cannot spawn search thread: {e}");
        }
    }

    fn apply_results(&self, g: u64, items: Vec<ResultItem>) {
        if g != self.gen.get() {
            return; // stale
        }
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let mut hints = Vec::new();
        for it in &items {
            let (row, hint) = build_row(it);
            self.list.append(&row);
            hints.push(hint);
        }
        self.list_wrap.set_visible(!items.is_empty());
        *self.hints.borrow_mut() = hints;
        *self.items.borrow_mut() = items;
        // Keep the confirm prompt only while its row is unchanged.
        self.confirm.set(None);
        self.select(0);
    }

    fn select(&self, i: usize) {
        let n = self.items.borrow().len();
        let i = if n == 0 { 0 } else { i.min(n - 1) };
        self.selected.set(i);
        if let Some(row) = self.list.row_at_index(i as i32) {
            self.list.select_row(Some(&row));
        }
        for (k, h) in self.hints.borrow().iter().enumerate() {
            h.set_visible(k == i);
        }
    }

    fn on_key(self: &Rc<Self>, a: KeyAction) {
        let n = self.items.borrow().len();
        match a {
            KeyAction::Up | KeyAction::Down => {
                self.cancel_confirm();
                self.select(keys::step(self.selected.get(), n, a == KeyAction::Down));
            }
            KeyAction::RunSelected(ai) => {
                if n > 0 {
                    self.run_index(self.selected.get(), ai);
                }
            }
            KeyAction::RunNth(i) => {
                if i < n {
                    self.run_index(i, 0);
                }
            }
            KeyAction::Escape => match keys::escape_outcome(self.confirm.get().is_some(), self.entry.text().is_empty()) {
                EscOutcome::CancelConfirm => self.cancel_confirm(),
                EscOutcome::ClearText => self.entry.set_text(""),
                EscOutcome::Hide => self.hide(),
            },
            KeyAction::Tab => {
                let fill = self.items.borrow().get(self.selected.get()).and_then(keys::tab_fill);
                if let Some(t) = fill {
                    self.entry.set_text(&t);
                    self.entry.set_position(-1);
                }
            }
        }
    }

    fn cancel_confirm(&self) {
        if self.confirm.replace(None).is_some() {
            self.reset_footer();
        }
    }

    fn run_index(self: &Rc<Self>, idx: usize, ai: usize) {
        let Some(item) = self.items.borrow().get(idx).cloned() else { return };
        let Some(action) = item.actions.get(ai) else { return };
        if action.needs_confirm() && self.confirm.get() != Some((idx, ai)) {
            self.confirm.set(Some((idx, ai)));
            self.select(idx);
            self.set_footer(&format!("Press Enter again to confirm: {}?", item.title), Some("confirm"));
            return;
        }
        self.confirm.set(None);
        match self.engine.run(&item, ai, self.platform.as_ref()) {
            Ok(out) => {
                if let Some(text) = &out.copied {
                    if let Some(d) = gdk::Display::default() {
                        d.clipboard().set_text(text);
                    }
                    self.toast("Copied", "toast");
                }
                if let Some(m) = &out.message {
                    self.toast(m, "toast");
                }
                if out.hide_window {
                    self.hide();
                }
            }
            Err(e) => self.toast(&format!("{e:#}"), "error"),
        }
    }

    fn set_footer(&self, text: &str, class: Option<&str>) {
        for c in ["toast", "error", "confirm"] {
            self.footer.remove_css_class(c);
        }
        if let Some(c) = class {
            self.footer.add_css_class(c);
        }
        self.footer.set_label(text);
    }

    fn reset_footer(&self) {
        self.toast_gen.set(self.toast_gen.get() + 1);
        self.set_footer(DEFAULT_FOOTER, None);
    }

    fn toast(self: &Rc<Self>, text: &str, class: &str) {
        let g = self.toast_gen.get() + 1;
        self.toast_gen.set(g);
        self.set_footer(text, Some(class));
        let ui = self.clone();
        glib::timeout_add_local_once(Duration::from_millis(2500), move || {
            if ui.toast_gen.get() == g {
                ui.set_footer(DEFAULT_FOOTER, None);
            }
        });
    }

    #[cfg(target_os = "linux")]
    fn place_window(&self) {
        let mut slot = self.x11.borrow_mut();
        if slot.is_none() {
            *slot = crate::placement::Placer::new();
        }
        if let Some(p) = slot.as_ref() {
            p.place(&self.window, WIDTH);
        }
    }

    #[cfg(feature = "layer-shell")]
    fn setup_layer_shell(&self) {
        // UNTESTED: gtk4-layer-shell is not available on the dev machine.
        use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
        if gtk4_layer_shell::is_supported() {
            self.window.init_layer_shell();
            self.window.set_layer(Layer::Overlay);
            self.window.set_keyboard_mode(KeyboardMode::OnDemand);
            self.window.set_anchor(Edge::Top, true);
            self.window.set_margin(Edge::Top, 160);
        }
    }
}

fn build_row(it: &ResultItem) -> (gtk::ListBoxRow, gtk::Box) {
    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hbox.set_valign(gtk::Align::Center);
    hbox.append(&icon_widget(&it.icon));

    let texts = gtk::Box::new(gtk::Orientation::Vertical, 1);
    texts.set_hexpand(true);
    texts.set_valign(gtk::Align::Center);
    let title = gtk::Label::builder().label(&it.title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    title.add_css_class("fudge-title");
    texts.append(&title);
    if !it.subtitle.is_empty() {
        let sub = gtk::Label::builder()
            .label(&it.subtitle)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        sub.add_css_class("fudge-subtitle");
        texts.append(&sub);
    }
    hbox.append(&texts);

    let hint = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    hint.set_valign(gtk::Align::Center);
    hint.set_visible(false);
    if let Some(a) = it.actions.first() {
        let l = gtk::Label::new(Some(&format!("↵ {}", a.label())));
        l.add_css_class("fudge-hint");
        hint.append(&l);
    }
    if let Some(a) = it.actions.get(1) {
        let l = gtk::Label::new(Some(&format!("Ctrl+↵ {}", a.label())));
        l.add_css_class("fudge-hint-alt");
        hint.append(&l);
    }
    hbox.append(&hint);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&hbox));
    (row, hint)
}

fn generic_icon() -> gtk::Image {
    let img = gtk::Image::from_icon_name("application-x-executable");
    img.set_pixel_size(32);
    img
}

fn icon_widget(icon: &Icon) -> gtk::Widget {
    let theme = gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
    let fixed = |w: &gtk::Widget| {
        w.set_size_request(32, 32);
        w.set_halign(gtk::Align::Center);
        w.set_valign(gtk::Align::Center);
    };
    let w: gtk::Widget = match icon {
        Icon::Named(n) if n.starts_with('/') => file_icon(std::path::Path::new(n)),
        Icon::Named(n) => {
            if theme.as_ref().map(|t| t.has_icon(n)).unwrap_or(false) {
                let img = gtk::Image::from_icon_name(n);
                img.set_pixel_size(32);
                img.upcast()
            } else {
                generic_icon().upcast()
            }
        }
        Icon::Path(p) => file_icon(p),
        Icon::Emoji(e) => {
            let l = gtk::Label::new(Some(e));
            l.add_css_class("fudge-emoji");
            l.upcast()
        }
        Icon::Color(c) => {
            let rgba = gdk::RGBA::parse(c.as_str()).unwrap_or(gdk::RGBA::new(0.5, 0.5, 0.5, 1.0));
            let da = gtk::DrawingArea::new();
            da.set_content_width(28);
            da.set_content_height(28);
            da.set_draw_func(move |_, cr, w, h| {
                let (w, h, r) = (w as f64, h as f64, 7.0);
                let pi = std::f64::consts::PI;
                cr.new_sub_path();
                cr.arc(w - r, r, r, -pi / 2.0, 0.0);
                cr.arc(w - r, h - r, r, 0.0, pi / 2.0);
                cr.arc(r, h - r, r, pi / 2.0, pi);
                cr.arc(r, r, r, pi, 3.0 * pi / 2.0);
                cr.close_path();
                cr.set_source_rgba(rgba.red() as f64, rgba.green() as f64, rgba.blue() as f64, rgba.alpha() as f64);
                let _ = cr.fill();
            });
            da.upcast()
        }
        Icon::None => generic_icon().upcast(),
    };
    fixed(&w);
    w
}

fn file_icon(p: &std::path::Path) -> gtk::Widget {
    if p.exists() {
        let img = gtk::Image::from_file(p);
        img.set_pixel_size(32);
        img.upcast()
    } else {
        generic_icon().upcast()
    }
}
