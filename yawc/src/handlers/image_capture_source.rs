use smithay::{
    output::WeakOutput,
    wayland::{
        foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelWeakHandle},
        image_capture_source::{
            ImageCaptureSource, ImageCaptureSourceHandler, OutputCaptureSourceHandler,
            OutputCaptureSourceState, ToplevelCaptureSourceHandler, ToplevelCaptureSourceState,
        },
    },
};

use crate::{YawcState, state::Backend};

#[derive(Clone, Debug)]
pub enum ImageCaptureSourceKind {
    Output(WeakOutput),
    Toplevel(ForeignToplevelWeakHandle),
}

impl<BackendData: Backend> ImageCaptureSourceHandler for YawcState<BackendData> {}
smithay::delegate_image_capture_source!(@<BackendData: Backend + 'static> YawcState<BackendData>);

impl<BackendData: Backend> OutputCaptureSourceHandler for YawcState<BackendData> {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.output_capture_source_state
    }

    fn output_source_created(
        &mut self,
        source: ImageCaptureSource,
        output: &smithay::output::Output,
    ) {
        source
            .user_data()
            .insert_if_missing(|| ImageCaptureSourceKind::Output(output.downgrade()));
    }
}
smithay::delegate_output_capture_source!(@<BackendData: Backend + 'static> YawcState<BackendData>);

impl<BackendData: Backend> ToplevelCaptureSourceHandler for YawcState<BackendData> {
    fn toplevel_capture_source_state(&mut self) -> &mut ToplevelCaptureSourceState {
        &mut self.toplevel_capture_source_state
    }

    fn toplevel_source_created(
        &mut self,
        source: ImageCaptureSource,
        toplevel: ForeignToplevelHandle,
    ) {
        source
            .user_data()
            .insert_if_missing(|| ImageCaptureSourceKind::Toplevel(toplevel.downgrade()));
    }
}
smithay::delegate_toplevel_capture_source!(@<BackendData: Backend + 'static> YawcState<BackendData>);
