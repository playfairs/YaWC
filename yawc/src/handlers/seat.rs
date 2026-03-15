use crate::{
    YawcState,
    focus::{KeyboardFocusTarget, PointerFocusTarget, Seat, WaylandFocus},
    state::Backend,
};
use smithay::{
    backend::input::TabletToolDescriptor,
    delegate_seat, delegate_tablet_manager,
    input::{SeatHandler, SeatState, keyboard::LedState, pointer::CursorImageStatus},
    reexports::wayland_server::Resource,
    wayland::{
        selection::{data_device::set_data_device_focus, primary_selection::set_primary_focus},
        tablet_manager::TabletSeatHandler,
    },
};

impl<BackendData: Backend> SeatHandler for YawcState<BackendData> {
    type KeyboardFocus = KeyboardFocusTarget;
    type PointerFocus = PointerFocusTarget;
    type TouchFocus = PointerFocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<YawcState<BackendData>> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, target: Option<&KeyboardFocusTarget>) {
        let dh = &self.display_handle;

        let wl_surface = target.and_then(WaylandFocus::wl_surface);

        let focus = wl_surface.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus.clone());
        set_primary_focus(dh, seat, focus);
    }
    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        self.backend_data.update_led_state(led_state)
    }
}
delegate_seat!(@<BackendData: Backend + 'static> YawcState<BackendData>);

impl<BackendData: Backend> TabletSeatHandler for YawcState<BackendData> {
    fn tablet_tool_image(&mut self, _tool: &TabletToolDescriptor, image: CursorImageStatus) {
        // TODO: tablet tools should have their own cursors
        self.cursor_status = image;
    }
}
delegate_tablet_manager!(@<BackendData: Backend + 'static> YawcState<BackendData>);
