use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use smithay::backend::input::{
    Axis, ButtonState, KeyState, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Rectangle, Serial, Transform};
use smithay::{
    backend::{
        input::{InputEvent, KeyboardKeyEvent},
        renderer::{
            element::{
                surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
                Kind,
            },
            utils::{draw_render_elements, on_commit_buffer_handler},
            Color32F, Frame, Renderer,
        },
    },
    delegate_compositor, delegate_data_device, delegate_seat, delegate_shm, delegate_xdg_shell,
    input::{keyboard::FilterResult, Seat, SeatHandler, SeatState},
    reexports::wayland_server::{protocol::wl_seat, Display},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            with_surface_tree_downward, CompositorClientState, CompositorHandler, CompositorState,
            SurfaceAttributes, TraversalAction,
        },
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
            },
            SelectionHandler,
        },
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
    },
};
use wayland_protocols::xdg::shell::server::xdg_toplevel;
use wayland_server::{
    backend::{ClientData, ClientId, DisconnectReason},
    protocol::{
        wl_buffer,
        wl_surface::{self, WlSurface},
    },
    Client, ListeningSocket, Resource,
};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const KEY_LEFTALT: u32 = 56;
const KEY_RIGHTALT: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    Move,
    Resize,
}

struct DragState {
    mode: DragMode,
    start_pointer: (f64, f64),
    start_window: (f64, f64),
    start_size: (i32, i32),
}

// ── wayland protocol impls ────────────────────────────────────────────────────

impl BufferHandler for App {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl XdgShellHandler for App {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|s| {
            s.states.set(xdg_toplevel::State::Activated);
        });
        surface.send_configure();
        let wl_surface = surface.wl_surface().clone();
        if let Some(kb) = self.seat.get_keyboard() {
            kb.set_focus(self, Some(wl_surface.clone()), Serial::from(0u32));
        }
        if let Some(ptr) = self.seat.get_pointer() {
            ptr.motion(
                self,
                Some((wl_surface, (0.0f64, 0.0f64).into())),
                &smithay::input::pointer::MotionEvent {
                    location: (0.0, 0.0).into(),
                    serial: Serial::from(0u32),
                    time: 0,
                },
            );
        }
    }

    fn new_popup(&mut self, _s: PopupSurface, _p: PositionerState) {}
    fn grab(&mut self, _s: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
    fn reposition_request(&mut self, _s: PopupSurface, _p: PositionerState, _t: u32) {}
}

impl SelectionHandler for App {
    type SelectionUserData = ();
}
impl DataDeviceHandler for App {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for App {}
impl ServerDndGrabHandler for App {}

impl CompositorHandler for App {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }
    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
    }
}

impl ShmHandler for App {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for App {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }
    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

struct App {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    seat_state: SeatState<Self>,
    data_device_state: DataDeviceState,
    seat: Seat<Self>,
}

delegate_xdg_shell!(App);
delegate_compositor!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_data_device!(App);

mod udev_mod;

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _id: ClientId) {
        println!("client connected");
    }
    fn disconnected(&self, _id: ClientId, _r: DisconnectReason) {
        println!("client disconnected");
    }
}

// ── shared helpers ────────────────────────────────────────────────────────────

fn handle_button_press(
    button: u32,
    pointer_pos: (f64, f64),
    alt_held: bool,
    drag: &mut Option<DragState>,
    xdg: &XdgShellState,
    window_positions: &HashMap<u32, (f64, f64)>,
) -> bool {
    if !alt_held || (button != BTN_LEFT && button != BTN_RIGHT) {
        return false;
    }
    if let Some(id) = xdg
        .toplevel_surfaces()
        .iter()
        .next()
        .map(|s| s.wl_surface().id().protocol_id())
    {
        let win_pos = *window_positions.get(&id).unwrap_or(&(0.0, 0.0));
        let win_size = xdg
            .toplevel_surfaces()
            .iter()
            .next()
            .and_then(|s| s.current_state().size)
            .map(|sz| (sz.w, sz.h))
            .unwrap_or((800, 600));
        *drag = Some(DragState {
            mode: if button == BTN_LEFT {
                DragMode::Move
            } else {
                DragMode::Resize
            },
            start_pointer: pointer_pos,
            start_window: win_pos,
            start_size: win_size,
        });
    }
    true
}

fn apply_motion(
    pointer_pos: (f64, f64),
    drag: &Option<DragState>,
    window_positions: &mut HashMap<u32, (f64, f64)>,
    xdg: &XdgShellState,
) -> bool {
    let ds = match drag {
        Some(d) => d,
        None => return false,
    };
    let dx = pointer_pos.0 - ds.start_pointer.0;
    let dy = pointer_pos.1 - ds.start_pointer.1;
    if let Some(id) = xdg
        .toplevel_surfaces()
        .iter()
        .next()
        .map(|s| s.wl_surface().id().protocol_id())
    {
        match ds.mode {
            DragMode::Move => {
                window_positions.insert(id, (ds.start_window.0 + dx, ds.start_window.1 + dy));
            }
            DragMode::Resize => {
                let nw = (ds.start_size.0 as f64 + dx).max(64.0) as i32;
                let nh = (ds.start_size.1 as f64 + dy).max(64.0) as i32;
                if let Some(tl) = xdg.toplevel_surfaces().iter().next().cloned() {
                    tl.with_pending_state(|s| {
                        s.size = Some((nw, nh).into());
                    });
                    tl.send_configure();
                }
            }
        }
    }
    true
}

pub fn send_frames_surface_tree(surface: &wl_surface::WlSurface, time: u32) {
    with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_surf, states, &()| {
            for cb in states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .frame_callbacks
                .drain(..)
            {
                cb.done(time);
            }
        },
        |_, _, &()| true,
    );
}

fn focus_and_forward_motion(
    pointer: &smithay::input::pointer::PointerHandle<App>,
    keyboard: &smithay::input::keyboard::KeyboardHandle<App>,
    state: &mut App,
    pointer_pos: (f64, f64),
    serial: Serial,
    time: u32,
) {
    if let Some(surface) = state
        .xdg_shell_state
        .toplevel_surfaces()
        .iter()
        .next()
        .map(|s| s.wl_surface().clone())
    {
        keyboard.set_focus(state, Some(surface.clone()), serial);
        pointer.motion(
            state,
            Some((surface, (0.0f64, 0.0f64).into())),
            &MotionEvent {
                location: pointer_pos.into(),
                serial,
                time,
            },
        );
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Minimal CLI parsing: support `--gpu <path|card>` or `-g <path|card>`
    let mut gpu_override: Option<String> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--gpu" | "-g" => {
                if i + 1 < args.len() {
                    gpu_override = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!(
                        "error: --gpu requires a value (e.g. --gpu /dev/dri/card1 or --gpu card1)"
                    );
                    std::process::exit(1);
                }
            }
            _ => {
                // ignore other args for now
                i += 1;
            }
        }
    }

    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        run_winit()
    } else {
        // Pass the optional GPU override into the udev backend
        run_udev(gpu_override)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WINIT BACKEND
// ═══════════════════════════════════════════════════════════════════════════════

fn run_winit() -> Result<(), Box<dyn std::error::Error>> {
    use ::winit::platform::pump_events::PumpStatus;
    use smithay::backend::renderer::gles::GlesRenderer;
    use smithay::backend::winit::{self, WinitEvent};

    let mut display: Display<App> = Display::new()?;
    let dh = display.handle();
    let compositor_state = CompositorState::new::<App>(&dh);
    let shm_state = ShmState::new::<App>(&dh, vec![]);
    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(&dh, "winit");
    let pointer = seat.add_pointer();

    let mut state = App {
        compositor_state,
        xdg_shell_state: XdgShellState::new::<App>(&dh),
        shm_state,
        seat_state,
        data_device_state: DataDeviceState::new::<App>(&dh),
        seat,
    };

    let listener = ListeningSocket::bind("wayland-5").unwrap();
    let mut clients = Vec::new();
    let (mut backend, mut winit_loop) = winit::init::<GlesRenderer>()?;
    let start_time = std::time::Instant::now();
    let keyboard = state
        .seat
        .add_keyboard(Default::default(), 200, 200)
        .unwrap();

    let mut pointer_pos: (f64, f64) = (0.0, 0.0);
    let mut window_positions: HashMap<u32, (f64, f64)> = HashMap::new();
    let mut drag: Option<DragState> = None;
    let mut alt_held = false;

    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", "wayland-5");
    }
    std::process::Command::new("alacritty").spawn().ok();

    loop {
        let status = winit_loop.dispatch_new_events(|event| match event {
            WinitEvent::Input(ev) => match ev {
                InputEvent::Keyboard { event } => {
                    let keycode = event.key_code();
                    let key_state = event.state();
                    if keycode == KEY_LEFTALT.into() || keycode == KEY_RIGHTALT.into() {
                        alt_held = key_state == KeyState::Pressed;
                    }
                    keyboard.input::<(), _>(
                        &mut state,
                        keycode,
                        key_state,
                        0.into(),
                        0,
                        |_, _, _| FilterResult::Forward,
                    );
                }
                InputEvent::PointerMotion { event } => {
                    use smithay::backend::input::Event as InputEv;
                    use smithay::backend::winit::WinitInput;
                    let delta = PointerMotionEvent::<WinitInput>::delta(&event);
                    let time = InputEv::<WinitInput>::time_msec(&event);
                    pointer_pos.0 += delta.x;
                    pointer_pos.1 += delta.y;
                    if apply_motion(
                        pointer_pos,
                        &drag,
                        &mut window_positions,
                        &state.xdg_shell_state,
                    ) {
                        return;
                    }
                    pointer.motion(
                        &mut state,
                        None,
                        &MotionEvent {
                            location: pointer_pos.into(),
                            serial: Serial::from(0u32),
                            time,
                        },
                    );
                }
                InputEvent::PointerButton { event } => {
                    use smithay::backend::input::Event as InputEv;
                    use smithay::backend::winit::WinitInput;
                    let btn_state = PointerButtonEvent::<WinitInput>::state(&event);
                    let button = PointerButtonEvent::<WinitInput>::button_code(&event);
                    let serial = Serial::from(0u32);
                    let time = InputEv::<WinitInput>::time_msec(&event);
                    if btn_state == ButtonState::Pressed {
                        focus_and_forward_motion(
                            &pointer,
                            &keyboard,
                            &mut state,
                            pointer_pos,
                            serial,
                            time,
                        );
                        if handle_button_press(
                            button,
                            pointer_pos,
                            alt_held,
                            &mut drag,
                            &state.xdg_shell_state,
                            &window_positions,
                        ) {
                            return;
                        }
                    }
                    if btn_state == ButtonState::Released && (alt_held || drag.is_some()) {
                        drag = None;
                        return;
                    }
                    pointer.button(
                        &mut state,
                        &ButtonEvent {
                            serial,
                            time,
                            button,
                            state: btn_state,
                        },
                    );
                }
                InputEvent::PointerAxis { event } => {
                    use smithay::backend::input::Event as InputEv;
                    use smithay::backend::winit::WinitInput;
                    let time = InputEv::<WinitInput>::time_msec(&event);
                    let source = PointerAxisEvent::<WinitInput>::source(&event);
                    let mut frame = AxisFrame::new(time).source(source);
                    if let Some(v) =
                        PointerAxisEvent::<WinitInput>::amount(&event, Axis::Horizontal)
                    {
                        frame = frame.value(Axis::Horizontal, v);
                    }
                    if let Some(v) = PointerAxisEvent::<WinitInput>::amount(&event, Axis::Vertical)
                    {
                        frame = frame.value(Axis::Vertical, v);
                    }
                    if let Some(v) =
                        PointerAxisEvent::<WinitInput>::amount_v120(&event, Axis::Horizontal)
                    {
                        frame = frame.v120(Axis::Horizontal, v as i32);
                    }
                    if let Some(v) =
                        PointerAxisEvent::<WinitInput>::amount_v120(&event, Axis::Vertical)
                    {
                        frame = frame.v120(Axis::Vertical, v as i32);
                    }
                    pointer.axis(&mut state, frame);
                }
                _ => {}
            },
            _ => {}
        });

        match status {
            PumpStatus::Continue => (),
            PumpStatus::Exit(_) => return Ok(()),
        }

        let size = backend.window_size();
        let damage = Rectangle::from_size(size);
        {
            let (renderer, mut framebuffer) = backend.bind().unwrap();
            let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = state
                .xdg_shell_state
                .toplevel_surfaces()
                .iter()
                .flat_map(|surface| {
                    let id = surface.wl_surface().id().protocol_id();
                    let pos = window_positions.get(&id).copied().unwrap_or((0.0, 0.0));
                    render_elements_from_surface_tree(
                        renderer,
                        surface.wl_surface(),
                        (pos.0 as i32, pos.1 as i32),
                        1.0,
                        1.0,
                        Kind::Unspecified,
                    )
                })
                .collect();

            let mut frame = renderer
                .render(&mut framebuffer, size, Transform::Flipped180)
                .unwrap();
            frame
                .clear(Color32F::new(0.1, 0.1, 0.1, 1.0), &[damage])
                .unwrap();
            draw_render_elements(&mut frame, 1.0, &elements, &[damage]).unwrap();
            let _ = frame.finish().unwrap();

            for surface in state.xdg_shell_state.toplevel_surfaces() {
                send_frames_surface_tree(
                    surface.wl_surface(),
                    start_time.elapsed().as_millis() as u32,
                );
            }
            if let Some(stream) = listener.accept()? {
                clients.push(
                    display
                        .handle()
                        .insert_client(stream, Arc::new(ClientState::default()))
                        .unwrap(),
                );
            }
            display.dispatch_clients(&mut state)?;
            display.flush_clients()?;
        }
        backend.submit(Some(&[damage])).unwrap();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UDEV / DRM BACKEND
// ═══════════════════════════════════════════════════════════════════════════════

fn run_udev(gpu_override: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Delegate to the modular udev implementation in `src/udev_mod/mod.rs`.
    // The module is declared near the top of this file (`mod udev_mod;`).
    crate::udev_mod::run_udev(gpu_override)
}

// ── DrmCompositor render ──────────────────────────────────────────────────────

fn udev_render(data: &mut UdevLoopData) -> Result<(), Box<dyn std::error::Error>> {
    use smithay::backend::drm::compositor::RenderFrameError;
    use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
    use smithay::backend::renderer::gles::GlesRenderer;

    // Collect window positions before borrowing drm_compositor
    let surfaces: Vec<(
        u32,
        (f64, f64),
        smithay::wayland::shell::xdg::ToplevelSurface,
    )> = data
        .app_state
        .xdg_shell_state
        .toplevel_surfaces()
        .iter()
        .map(|s| {
            let id = s.wl_surface().id().protocol_id();
            let pos = data
                .window_positions
                .get(&id)
                .copied()
                .unwrap_or((0.0, 0.0));
            (id, pos, s.clone())
        })
        .collect();

    // Build the render elements first (scoped mutable borrow of the renderer).
    let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = {
        let tmp_renderer = &mut data.renderer;
        surfaces
            .iter()
            .flat_map(|(_, pos, surface)| {
                render_elements_from_surface_tree(
                    tmp_renderer,
                    surface.wl_surface(),
                    (pos.0 as i32, pos.1 as i32),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                )
            })
            .collect()
    };

    // Call the newer render_frame signature: (renderer, elements_slice, clear_color, frame_flags)
    let render_result = data.drm_compositor.render_frame(
        &mut data.renderer,
        &elements,
        Color32F::new(0.1, 0.1, 0.1, 1.0),
        smithay::backend::drm::compositor::FrameFlags::DEFAULT,
    );

    match render_result {
        Ok(result) => {
            // If the frame result indicates there is something to present, queue it.
            if !result.is_empty {
                data.drm_compositor.queue_frame(())?;
            }
        }
        Err(RenderFrameError::PrepareFrame(err)) => {
            eprintln!("PrepareFrame error: {err:?}");
        }
        Err(RenderFrameError::RenderFrame(err)) => {
            eprintln!("RenderFrame error: {err:?}");
        }
    }

    Ok(())
}

// ── generic input handler (libinput backend) ──────────────────────────────────

fn process_input_event<B: smithay::backend::input::InputBackend>(
    event: InputEvent<B>,
    pointer: &smithay::input::pointer::PointerHandle<App>,
    keyboard: &smithay::input::keyboard::KeyboardHandle<App>,
    state: &mut App,
    pointer_pos: &mut (f64, f64),
    window_positions: &mut HashMap<u32, (f64, f64)>,
    drag: &mut Option<DragState>,
    alt_held: &mut bool,
) {
    match event {
        InputEvent::Keyboard { event } => {
            let keycode = event.key_code();
            let key_state = event.state();
            if keycode == KEY_LEFTALT.into() || keycode == KEY_RIGHTALT.into() {
                *alt_held = key_state == KeyState::Pressed;
            }
            keyboard.input::<(), _>(state, keycode, key_state, 0.into(), 0, |_, _, _| {
                FilterResult::Forward
            });
        }
        InputEvent::PointerMotion { event } => {
            use smithay::backend::input::Event as E;
            let delta = event.delta();
            let time = E::time_msec(&event);
            pointer_pos.0 += delta.x;
            pointer_pos.1 += delta.y;
            if apply_motion(*pointer_pos, drag, window_positions, &state.xdg_shell_state) {
                return;
            }
            pointer.motion(
                state,
                None,
                &MotionEvent {
                    location: (*pointer_pos).into(),
                    serial: Serial::from(0u32),
                    time,
                },
            );
        }
        InputEvent::PointerButton { event } => {
            use smithay::backend::input::Event as E;
            let btn_state = event.state();
            let button = event.button_code();
            let serial = Serial::from(0u32);
            let time = E::time_msec(&event);
            if btn_state == ButtonState::Pressed {
                focus_and_forward_motion(pointer, keyboard, state, *pointer_pos, serial, time);
                if handle_button_press(
                    button,
                    *pointer_pos,
                    *alt_held,
                    drag,
                    &state.xdg_shell_state,
                    window_positions,
                ) {
                    return;
                }
            }
            if btn_state == ButtonState::Released && (*alt_held || drag.is_some()) {
                *drag = None;
                return;
            }
            pointer.button(
                state,
                &ButtonEvent {
                    serial,
                    time,
                    button,
                    state: btn_state,
                },
            );
        }
        InputEvent::PointerAxis { event } => {
            use smithay::backend::input::Event as E;
            use smithay::backend::input::PointerAxisEvent as PAE;
            let time = E::time_msec(&event);
            let source = event.source();
            let mut frame = AxisFrame::new(time).source(source);
            if let Some(v) = event.amount(Axis::Horizontal) {
                frame = frame.value(Axis::Horizontal, v);
            }
            if let Some(v) = event.amount(Axis::Vertical) {
                frame = frame.value(Axis::Vertical, v);
            }
            if let Some(v) = event.amount_v120(Axis::Horizontal) {
                frame = frame.v120(Axis::Horizontal, v as i32);
            }
            if let Some(v) = event.amount_v120(Axis::Vertical) {
                frame = frame.v120(Axis::Vertical, v as i32);
            }
            pointer.axis(state, frame);
        }
        _ => {}
    }
}

// ── udev loop data ────────────────────────────────────────────────────────────

type UdevDrmCompositor = smithay::backend::drm::compositor::DrmCompositor<
    smithay::backend::allocator::gbm::GbmAllocator<smithay::backend::drm::DrmDeviceFd>,
    std::sync::Arc<
        std::sync::Mutex<
            smithay::backend::drm::exporter::gbm::GbmFramebufferExporter<
                smithay::backend::drm::DrmDeviceFd,
            >,
        >,
    >,
    (),
    smithay::backend::drm::DrmDeviceFd,
>;

struct UdevLoopData {
    display: Display<App>,
    app_state: App,
    drm_compositor: UdevDrmCompositor,
    renderer: smithay::backend::renderer::gles::GlesRenderer,
    pointer: smithay::input::pointer::PointerHandle<App>,
    keyboard: smithay::input::keyboard::KeyboardHandle<App>,
    listener: ListeningSocket,
    clients: Vec<wayland_server::Client>,
    pointer_pos: (f64, f64),
    window_positions: HashMap<u32, (f64, f64)>,
    drag: Option<DragState>,
    alt_held: bool,
    start_time: std::time::Instant,
    paused: bool,
}
