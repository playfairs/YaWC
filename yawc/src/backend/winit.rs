use std::{
    sync::{Mutex, atomic::Ordering},
    time::Duration,
};

#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;
#[cfg(debug_assertions)]
use smithay::{
    backend::{allocator::Fourcc, renderer::ImportMem},
    reexports::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
};

use smithay::{
    backend::{
        SwapBuffersError,
        allocator::dmabuf::Dmabuf,
        egl::EGLDevice,
        renderer::{
            ImportDma, ImportMemWl,
            damage::{Error as OutputDamageTrackerError, OutputDamageTracker},
            element::AsRenderElements,
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    delegate_dmabuf,
    input::{
        keyboard::LedState,
        pointer::{CursorImageAttributes, CursorImageStatus},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::{Display, protocol::wl_surface},
        winit::platform::pump_events::PumpStatus,
    },
    utils::{IsAlive, Scale, Transform},
    wayland::{
        compositor,
        dmabuf::{
            DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState,
            ImportNotifier,
        },
        presentation::Refresh,
    },
};
use tracing::{error, info, warn};

use crate::state::{Backend, YawcState, take_presentation_feedback};
use crate::{drawing::*, render::*};

pub const OUTPUT_NAME: &str = "winit";

pub struct WinitData {
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
    dmabuf_state: (DmabufState, DmabufGlobal, Option<DmabufFeedback>),
    full_redraw: u8,
    #[cfg(debug_assertions)]
    pub fps: Fps,
}

impl DmabufHandler for YawcState<WinitData> {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.backend_data.dmabuf_state.0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .backend_data
            .backend
            .renderer()
            .import_dmabuf(&dmabuf, None)
            .is_ok()
        {
            let _ = notifier.successful::<YawcState<WinitData>>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(YawcState<WinitData>);

impl Backend for WinitData {
    fn seat_name(&self) -> String {
        String::from("winit")
    }
    fn reset_buffers(&mut self, _output: &Output) {
        self.full_redraw = 4;
    }
    fn early_import(&mut self, _surface: &wl_surface::WlSurface) {}
    fn update_led_state(&mut self, _led_state: LedState) {}
}

pub fn run_winit() {
    let mut event_loop = EventLoop::try_new().unwrap();
    let display = Display::new().unwrap();
    let mut display_handle = display.handle();

    #[cfg_attr(not(feature = "egl"), allow(unused_mut))]
    let (mut backend, mut winit) = match winit::init::<GlesRenderer>() {
        Ok(ret) => ret,
        Err(err) => {
            error!("Failed to initialize Winit backend: {}", err);
            return;
        }
    };
    let size = backend.window_size();

    let mode = Mode {
        size,
        refresh: 60_000,
    };
    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
            serial_number: "Unknown".into(),
        },
    );
    let _global = output.create_global::<YawcState<WinitData>>(&display.handle());
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    #[cfg(debug_assertions)]
    let fps_texture = {
        use png::{Decoder, Transformations};
        use std::io::Cursor;

        let mut decoder = Decoder::new(Cursor::new(FPS_NUMBERS_PNG));
        decoder.set_transformations(Transformations::EXPAND);

        let mut reader = decoder.read_info().unwrap();

        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();

        let pixels = &buf[..info.buffer_size()];

        backend
            .renderer()
            .import_memory(
                pixels,
                Fourcc::Abgr8888,
                (info.width as i32, info.height as i32).into(),
                false,
            )
            .expect("Unable to upload FPS texture")
    };

    #[cfg(debug_assertions)]
    let mut fps_element = FpsElement::new(fps_texture);

    let render_node = EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|device| device.try_get_render_node());

    let dmabuf_default_feedback = match render_node {
        Ok(Some(node)) => {
            let dmabuf_formats = backend.renderer().dmabuf_formats();
            let dmabuf_default_feedback = DmabufFeedbackBuilder::new(node.dev_id(), dmabuf_formats)
                .build()
                .unwrap();
            Some(dmabuf_default_feedback)
        }
        Ok(None) => {
            warn!("failed to query render node, dmabuf will use v3");
            None
        }
        Err(err) => {
            warn!(?err, "failed to egl device for display, dmabuf will use v3");
            None
        }
    };

    // if we failed to build dmabuf feedback we fall back to dmabuf v3
    // Note: egl on Mesa requires either v4 or wl_drm (initialized with bind_wl_display)
    let dmabuf_state = if let Some(default_feedback) = dmabuf_default_feedback {
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global = dmabuf_state
            .create_global_with_default_feedback::<YawcState<WinitData>>(
                &display.handle(),
                &default_feedback,
            );
        (dmabuf_state, dmabuf_global, Some(default_feedback))
    } else {
        let dmabuf_formats = backend.renderer().dmabuf_formats();
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global =
            dmabuf_state.create_global::<YawcState<WinitData>>(&display.handle(), dmabuf_formats);
        (dmabuf_state, dmabuf_global, None)
    };

    #[cfg(feature = "egl")]
    if backend
        .renderer()
        .bind_wl_display(&display.handle())
        .is_ok()
    {
        info!("EGL hardware-acceleration enabled");
    };

    let data = {
        let damage_tracker = OutputDamageTracker::from_output(&output);

        WinitData {
            backend,
            damage_tracker,
            dmabuf_state,
            full_redraw: 0,
            #[cfg(debug_assertions)]
            fps: Fps::default(),
        }
    };
    let mut state = YawcState::init(display, event_loop.handle(), data, true);
    state
        .shm_state
        .update_formats(state.backend_data.backend.renderer().shm_formats());
    state.space.map_output(&output, (0, 0));

    #[cfg(feature = "xwayland")]
    state.start_xwayland();

    info!("Initialization completed, starting the main loop.");

    let mut pointer_element = PointerElement::default();

    while state.running.load(Ordering::SeqCst) {
        let status = winit.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                // We only have one output
                let output = state.space.outputs().next().unwrap().clone();
                state.space.map_output(&output, (0, 0));
                let mode = Mode {
                    size,
                    refresh: 60_000,
                };
                output.change_current_state(Some(mode), None, None, None);
                output.set_preferred(mode);
                crate::shell::fixup_positions(&mut state.space, state.pointer.current_location());
            }
            WinitEvent::Input(event) => state.process_input_event_windowed(event, OUTPUT_NAME),
            _ => (),
        });

        if let PumpStatus::Exit(_) = status {
            state.running.store(false, Ordering::SeqCst);
            break;
        }

        // drawing logic
        {
            let now = state.clock.now();
            let frame_target = now
                + output
                    .current_mode()
                    .map(|mode| Duration::from_secs_f64(1_000f64 / mode.refresh as f64))
                    .unwrap_or_default();
            state.pre_repaint(&output, frame_target);

            let backend = &mut state.backend_data.backend;

            // draw the cursor as relevant
            // reset the cursor if the surface is no longer alive
            let mut reset = false;
            if let CursorImageStatus::Surface(ref surface) = state.cursor_status {
                reset = !surface.alive();
            }
            if reset {
                state.cursor_status = CursorImageStatus::default_named();
            }
            let cursor_visible = !matches!(state.cursor_status, CursorImageStatus::Surface(_));

            pointer_element.set_status(state.cursor_status.clone());

            #[cfg(debug_assertions)]
            let fps = state.backend_data.fps.avg().round() as u32;
            #[cfg(debug_assertions)]
            fps_element.update_fps(fps);

            let full_redraw = &mut state.backend_data.full_redraw;
            *full_redraw = full_redraw.saturating_sub(1);
            let space = &mut state.space;
            let damage_tracker = &mut state.backend_data.damage_tracker;
            let show_window_preview = state.show_window_preview;

            let dnd_icon = state.dnd_icon.as_ref();

            let scale = Scale::from(output.current_scale().fractional_scale());
            let cursor_hotspot =
                if let CursorImageStatus::Surface(ref surface) = state.cursor_status {
                    compositor::with_states(surface, |states| {
                        states
                            .data_map
                            .get::<Mutex<CursorImageAttributes>>()
                            .unwrap()
                            .lock()
                            .unwrap()
                            .hotspot
                    })
                } else {
                    (0, 0).into()
                };
            let cursor_pos = state.pointer.current_location();

            #[cfg(debug_assertions)]
            let mut renderdoc = state.renderdoc.as_mut();

            let age = if *full_redraw > 0 {
                0
            } else {
                backend.buffer_age().unwrap_or(0)
            };
            #[cfg(debug_assertions)]
            let window_handle = backend
                .window()
                .window_handle()
                .map(|handle| {
                    if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                        handle.surface.as_ptr()
                    } else {
                        std::ptr::null_mut()
                    }
                })
                .unwrap_or_else(|_| std::ptr::null_mut());
            let render_res = backend.bind().and_then(|(renderer, mut fb)| {
                #[cfg(debug_assertions)]
                if let Some(renderdoc) = renderdoc.as_mut() {
                    renderdoc.start_frame_capture(
                        renderer.egl_context().get_context_handle(),
                        window_handle,
                    );
                }

                let mut elements = Vec::<CustomRenderElements<GlesRenderer>>::new();

                elements.extend(
                    pointer_element.render_elements(
                        renderer,
                        (cursor_pos - cursor_hotspot.to_f64())
                            .to_physical(scale)
                            .to_i32_round(),
                        scale,
                        1.0,
                    ),
                );

                // draw the dnd icon if any
                if let Some(icon) = dnd_icon {
                    let dnd_icon_pos = (cursor_pos + icon.offset.to_f64())
                        .to_physical(scale)
                        .to_i32_round();
                    if icon.surface.alive() {
                        elements.extend(AsRenderElements::<GlesRenderer>::render_elements(
                            &smithay::desktop::space::SurfaceTree::from_surface(&icon.surface),
                            renderer,
                            dnd_icon_pos,
                            scale,
                            1.0,
                        ));
                    }
                }

                #[cfg(debug_assertions)]
                elements.push(CustomRenderElements::Fps(fps_element.clone()));

                render_output(
                    &output,
                    space,
                    elements,
                    renderer,
                    &mut fb,
                    damage_tracker,
                    age,
                    show_window_preview,
                )
                .map_err(|err| match err {
                    OutputDamageTrackerError::Rendering(err) => err.into(),
                    _ => unreachable!(),
                })
            });

            match render_res {
                Ok(render_output_result) => {
                    let has_rendered = render_output_result.damage.is_some();
                    if let Some(damage) = render_output_result.damage
                        && let Err(err) = backend.submit(Some(damage))
                    {
                        warn!("Failed to submit buffer: {}", err);
                    }

                    #[cfg(debug_assertions)]
                    if let Some(renderdoc) = renderdoc.as_mut() {
                        renderdoc.end_frame_capture(
                            backend.renderer().egl_context().get_context_handle(),
                            backend
                                .window()
                                .window_handle()
                                .map(|handle| {
                                    if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                                        handle.surface.as_ptr()
                                    } else {
                                        std::ptr::null_mut()
                                    }
                                })
                                .unwrap_or_else(|_| std::ptr::null_mut()),
                        );
                    }

                    backend.window().set_cursor_visible(cursor_visible);

                    let states = render_output_result.states;
                    if has_rendered {
                        let mut output_presentation_feedback =
                            take_presentation_feedback(&output, &state.space, &states);
                        output_presentation_feedback.presented(
                            frame_target,
                            output
                                .current_mode()
                                .map(|mode| {
                                    Refresh::fixed(Duration::from_secs_f64(
                                        1_000f64 / mode.refresh as f64,
                                    ))
                                })
                                .unwrap_or(Refresh::Unknown),
                            0,
                            wp_presentation_feedback::Kind::Vsync,
                        )
                    }

                    // Send frame events so that client start drawing their next frame
                    state.post_repaint(&output, frame_target, None, &states);
                }
                Err(SwapBuffersError::ContextLost(err)) => {
                    #[cfg(debug_assertions)]
                    if let Some(renderdoc) = renderdoc.as_mut() {
                        renderdoc.discard_frame_capture(
                            backend.renderer().egl_context().get_context_handle(),
                            backend
                                .window()
                                .window_handle()
                                .map(|handle| {
                                    if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                                        handle.surface.as_ptr()
                                    } else {
                                        std::ptr::null_mut()
                                    }
                                })
                                .unwrap_or_else(|_| std::ptr::null_mut()),
                        );
                    }

                    error!("Critical Rendering Error: {}", err);
                    state.running.store(false, Ordering::SeqCst);
                }
                Err(err) => warn!("Rendering error: {}", err),
            }
        }

        let result = event_loop.dispatch(Some(Duration::from_millis(1)), &mut state);
        if result.is_err() {
            state.running.store(false, Ordering::SeqCst);
        } else {
            state.space.refresh();
            state.popups.cleanup();
            display_handle.flush_clients().unwrap();
        }

        #[cfg(debug_assertions)]
        state.backend_data.fps.tick();
    }
}
