/* YaWC/compositor/src/udev_mod/mod.rs
 *
 * Incremental, modular udev-based DRM path extracted from the big main file.
 *
 * This module provides `run_udev` which:
 *  - opens a DRM device (optionally overridden by caller)
 *  - initializes EGL/GLES renderer
 *  - tries to reliably find a connected connector + crtc using DrmScanner first,
 *    falling back to connector.encoders()/get_encoder() and, if needed, scanning
 *    other /dev/dri/card* devices to find the device that actually owns the
 *    connector with an encoder->crtc chain.
 *  - sets up a simple DrmCompositor surface using GBM and exposes dmabuf feedback.
 *
 * The implementation intentionally focuses on being an incremental, less-invasive
 * replacement for the ad-hoc single-device selection. It keeps DMABUF support
 * for clients (collect renderer dmabuf formats, build a default DmabufFeedback).
 *
 * NOTE: This file mirrors the approach used in the upstream Anvil udev path but
 * is intentionally smaller and self-contained for use in YaWC. You will still
 * want to wire this module into `main.rs` (replace previous `run_udev` there and
 * call this `udev_mod::run_udev(...)`).
 */

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use smithay::{
    backend::{
        allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        allocator::Fourcc,
        drm::{DrmDevice, DrmDeviceFd},
        egl::{EGLContext, EGLDisplay},
        renderer::gles::GlesRenderer,
        session::libseat::LibSeatSession,
        session::Session,
        udev::primary_gpu,
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::ModeTypeFlags,
        rustix::fs::OFlags,
        wayland_server::Display,
    },
    utils::{DeviceFd, Transform},
    wayland::dmabuf::DmabufFeedbackBuilder,
};
use smithay::backend::drm::compositor::{DrmCompositor, FrameFlags};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::{DrmSurface, DrmNode};
use smithay_drm_extras::{display_info, drm_scanner::DrmScanner, drm_scanner::DrmScanEvent};

use tracing::{error, info, warn};

/// Run the udev/DRM compositor path.
///
/// - `gpu_override` accepts either an absolute device path like `/dev/dri/card1`
///   or a short name like `card1`. If None, `primary_gpu()` is used to pick a device.
///
/// This is intentionally a relatively small, self-contained function that uses
/// DrmScanner to robustly find connector->crtc pairs, and falls back to scanning
/// other cards if needed.
pub fn run_udev(gpu_override: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Create a minimal event loop & display for compositor wiring (kept simple here).
    let mut _event_loop: EventLoop<()> = EventLoop::try_new()?;
    let display: Display<()> = Display::new()?;

    // Initialize libseat session to open DRM nodes (so session ownership is respected).
    let (session, _notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();
    info!("Starting udev DRM path for seat '{}'", seat_name);

    // Normalize override or find a primary GPU path via primary_gpu()
    let primary_path: PathBuf = if let Some(ov) = gpu_override.as_ref() {
        if ov.starts_with('/') {
            PathBuf::from(ov)
        } else if ov.starts_with("card") {
            PathBuf::from("/dev/dri").join(ov)
        } else {
            PathBuf::from(ov)
        }
    } else {
        // primary_gpu returns Option<Result<PathBuf, _>> or similar; try to handle both
        let dev = primary_gpu(&seat_name).ok().flatten().ok_or("no primary GPU found")?;
        dev.into()
    };

    info!("Attempting to open DRM device at {}", primary_path.display());

    // Try to open the primary device via session
    let opened = session.open(
        &primary_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;

    let drm_fd = DrmDeviceFd::new(DeviceFd::from(opened));
    let (drm, _drm_notifier) = DrmDevice::new(drm_fd.clone(), true)?;
    let gbm = GbmDevice::new(drm_fd.clone())?;

    // Initialize EGL + GLES renderer
    let egl = unsafe { EGLDisplay::new(gbm.clone())? };
    let ctx = EGLContext::new(&egl)?;
    let mut renderer = unsafe { GlesRenderer::new(ctx)? };

    // Setup DMABUF default feedback from renderer formats so clients can be informed.
    let dmabuf_formats = renderer.dmabuf_formats();
    let default_feedback = DmabufFeedbackBuilder::new(primary_path.clone().into_os_string().into_string().unwrap_or_default().into(), dmabuf_formats)
        .build()
        .ok();

    // Use a DrmScanner to probe connectors/crtcs on the opened device.
    let scanner = DrmScanner::new();
    let mut chosen_connector = None;
    let mut chosen_crtc = None;

    match scanner.scan_connectors(&drm) {
        Ok(scan_result) => {
            // Prefer entries that already resolved to a CRTC
            for ev in scan_result.iter() {
                match ev {
                    DrmScanEvent::Connected { connector, crtc: Some(crtc) } => {
                        chosen_connector = Some(connector.clone());
                        chosen_crtc = Some(*crtc);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(err) => {
            warn!("DrmScanner failed to scan connectors on {}: {:?}", primary_path.display(), err);
        }
    }

    // If scanner didn't yield a connector with CRTC, fallback to the connector list and encoders
    if chosen_crtc.is_none() {
        let res = drm.resource_handles()?;
        // pick the first connected connector that has an encoder->crtc on this device
        for &h in res.connectors() {
            if let Ok(info) = drm.get_connector(h, false) {
                use smithay::reexports::drm::control::connector::State;
                if info.state() != State::Connected {
                    continue;
                }
                // try current_encoder
                if let Some(enc_h) = info.current_encoder() {
                    if let Ok(enc) = drm.get_encoder(enc_h) {
                        if let Some(crtc_h) = enc.crtc() {
                            chosen_connector = Some(info.clone());
                            chosen_crtc = Some(crtc_h);
                            break;
                        }
                    }
                }
                // try encoder list
                for &enc_h in info.encoders() {
                    if let Ok(enc) = drm.get_encoder(enc_h) {
                        if let Some(crtc_h) = enc.crtc() {
                            chosen_connector = Some(info.clone());
                            chosen_crtc = Some(crtc_h);
                            break;
                        }
                    }
                }
                if chosen_crtc.is_some() {
                    break;
                }
            }
        }
    }

    // If still not found, attempt to scan other /dev/dri/card* devices. This is a more
    // invasive fallback but often necessary on multi-GPU systems where the primary node
    // doesn't actually own the connector.
    if chosen_crtc.is_none() {
        if let Ok(entries) = fs::read_dir("/dev/dri") {
            for entry_res in entries {
                if let Ok(entry) = entry_res {
                    let fname = entry.file_name().into_string().unwrap_or_default();
                    if !fname.starts_with("card") {
                        continue;
                    }
                    let candidate_path = entry.path();
                    if candidate_path == primary_path {
                        // already tried
                        continue;
                    }
                    // Try opening via session (note: may fail due to permissions/seat ownership)
                    match session.open(&candidate_path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK) {
                        Ok(fd) => {
                            let other_fd = DrmDeviceFd::new(DeviceFd::from(fd));
                            if let Ok((other_drm, _)) = DrmDevice::new(other_fd.clone(), true) {
                                // scan connectors on other_drm
                                if let Ok(res2) = other_drm.resource_handles() {
                                    for &h in res2.connectors() {
                                        if let Ok(info2) = other_drm.get_connector(h, false) {
                                            use smithay::reexports::drm::control::connector::State;
                                            if info2.state() != State::Connected {
                                                continue;
                                            }
                                            // try current_encoder
                                            if let Some(enc_h) = info2.current_encoder() {
                                                if let Ok(enc2) = other_drm.get_encoder(enc_h) {
                                                    if let Some(crtc_h) = enc2.crtc() {
                                                        info!("Found connector owned by {} at {} -> crtc {:?}", candidate_path.display(), info2.handle(), crtc_h);
                                                        chosen_connector = Some(info2.clone());
                                                        chosen_crtc = Some(crtc_h);
                                                        break;
                                                    }
                                                }
                                            }
                                            for &enc_h in info2.encoders() {
                                                if let Ok(enc2) = other_drm.get_encoder(enc_h) {
                                                    if let Some(crtc_h) = enc2.crtc() {
                                                        info!("Found connector owned by {} at {} -> crtc {:?}", candidate_path.display(), info2.handle(), crtc_h);
                                                        chosen_connector = Some(info2.clone());
                                                        chosen_crtc = Some(crtc_h);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(_e) => {
                            // ignore devices we can't open
                        }
                    }
                }
                if chosen_crtc.is_some() {
                    break;
                }
            }
        }
    }

    // If we still don't have a connector/crtc, emit helpful diagnostics and bail out.
    let connector = match chosen_connector {
        Some(conn) => conn,
        None => {
            error!("Could not find a usable connector on device {}.", primary_path.display());
            error!("Connector/encoder diagnostics:");
            if let Ok(res) = drm.resource_handles() {
                for &h in res.connectors() {
                    if let Ok(info) = drm.get_connector(h, false) {
                        error!(" connector {:?}: state={:?}, encoders={:?}", info.handle(), info.state(), info.encoders());
                    }
                }
            }
            error!("Try running the compositor with `--gpu /dev/dri/cardX` pointing to the card that exposes your monitor.");
            return Err("no usable connector found".into());
        }
    };

    let crtc = match chosen_crtc {
        Some(c) => c,
        None => {
            error!("Found connector {:?} but unable to resolve a CRTC.", connector.handle());
            return Err("no crtc for connector".into());
        }
    };

    // Choose a mode on the connector (preferred if available)
    let mode = connector
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| connector.modes().first())
        .copied()
        .ok_or("no mode on connector")?;

    let (w, h) = mode.size();
    info!("Selected mode: {}x{}@{}", w, h, mode.vrefresh());

    // Create DRM surface & planes and GBM allocator, then DrmCompositor
    let drm_surface = drm.create_surface(crtc, mode, &[connector.handle()])?;
    let planes = drm.planes(&crtc)?;
    let gbm_alloc = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);

    let fb_exporter = std::sync::Arc::new(std::sync::Mutex::new(GbmFramebufferExporter::new(gbm.clone(), None)));

    let output = Output::new(
        "udev-output".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Generic".into(),
            model: "DRM".into(),
        },
    );
    let wl_mode = WlMode {
        size: (w as i32, h as i32).into(),
        refresh: mode.vrefresh() as i32 * 1000,
    };
    output.change_current_state(Some(wl_mode), Some(Transform::Normal), None, None);
    output.set_preferred(wl_mode);

    // Renderer formats that the renderer can scanout
    let renderer_formats = renderer.dmabuf_formats().into_iter().collect::<std::collections::HashSet<_>>();

    let buf_size: smithay::utils::Size<u32, smithay::utils::Buffer> = (w as u32, h as u32).into();

    let drm_compositor: DrmCompositor<
        GbmAllocator<DrmDeviceFd>,
        GbmDevice<DrmDeviceFd>,
        Option<()>,
        DrmDeviceFd,
    > = DrmCompositor::new(
        &output,
        drm_surface,
        Some(planes),
        gbm_alloc,
        fb_exporter,
        [Fourcc::Xrgb8888, Fourcc::Argb8888],
        renderer_formats,
        buf_size,
        Some(gbm.clone()),
    )?;

    // At this point we have a compositor and output: continue with the rest of the
    // compositor initialization (Wayland display, inputs, render loop) in main.
    //
    // For this incremental module we stop here and return Ok(()). The caller is
    // expected to take the `drm_compositor` and continue wiring it into the
    // compositor runtime (or we could extend this module to return a struct with
    // the relevant pieces). For the current purpose, returning means we succeeded
    // in selecting a device, mode and creating a DrmCompositor instance.
    info!("run_udev succeeded in creating DrmCompositor on device {}", primary_path.display());

    // Keep compositor alive briefly or hand back to caller in a real integration.
    // Here we just sleep shortly to allow the logs to be read when testing.
    std::thread::sleep(Duration::from_millis(20));

    Ok(())
}
