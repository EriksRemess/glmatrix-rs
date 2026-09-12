//! Minimal, version-1 color-management wire descriptions. No generated C library
//! or additional build dependency is needed. Unused constructor types are null.

use super::super::wayland::{self, WlInterface, WlMessage};
use std::ffi::CStr;
use std::ptr;
use std::sync::OnceLock;

pub struct Interfaces {
    pub manager: *mut WlInterface,
    pub surface: *mut WlInterface,
    pub feedback: *mut WlInterface,
    pub params: *mut WlInterface,
    pub description: *mut WlInterface,
    pub info: *mut WlInterface,
}

fn allocate(name: &'static CStr) -> *mut WlInterface {
    Box::into_raw(Box::new(WlInterface {
        name: name.as_ptr(),
        version: 1,
        method_count: 0,
        methods: ptr::null(),
        event_count: 0,
        events: ptr::null(),
    }))
}

fn message(
    name: &'static CStr,
    signature: &'static CStr,
    types: &[*const WlInterface],
) -> WlMessage {
    let types = if types.is_empty() {
        // Primitive arguments still need one null type entry each.
        vec![ptr::null(); signature.to_bytes().len()]
    } else {
        types.to_vec()
    };
    WlMessage {
        name: name.as_ptr(),
        signature: signature.as_ptr(),
        types: Box::leak(types.into_boxed_slice()).as_ptr(),
    }
}

unsafe fn define(interface: *mut WlInterface, methods: Vec<WlMessage>, events: Vec<WlMessage>) {
    (*interface).method_count = methods.len() as i32;
    (*interface).methods = Box::leak(methods.into_boxed_slice()).as_ptr();
    (*interface).event_count = events.len() as i32;
    (*interface).events = Box::leak(events.into_boxed_slice()).as_ptr();
}

pub fn interfaces() -> &'static Interfaces {
    // The descriptors are immutable after initialization and live for the
    // process lifetime, as required by libwayland. Store the published address
    // rather than making the general-purpose raw FFI types Sync.
    static ADDRESS: OnceLock<usize> = OnceLock::new();
    let address = *ADDRESS.get_or_init(|| unsafe {
        let p = Interfaces {
            manager: allocate(c"wp_color_manager_v1"),
            surface: allocate(c"wp_color_management_surface_v1"),
            feedback: allocate(c"wp_color_management_surface_feedback_v1"),
            params: allocate(c"wp_image_description_creator_params_v1"),
            description: allocate(c"wp_image_description_v1"),
            info: allocate(c"wp_image_description_info_v1"),
        };
        define(
            p.manager,
            vec![
                message(c"destroy", c"", &[]),
                message(c"get_output", c"no", &[ptr::null(), ptr::null()]),
                message(
                    c"get_surface",
                    c"no",
                    &[p.surface, &wayland::wl_surface_interface],
                ),
                message(
                    c"get_surface_feedback",
                    c"no",
                    &[p.feedback, &wayland::wl_surface_interface],
                ),
                message(c"create_icc_creator", c"n", &[]),
                message(c"create_parametric_creator", c"n", &[p.params]),
                message(c"create_windows_scrgb", c"n", &[p.description]),
            ],
            vec![
                message(c"supported_intent", c"u", &[]),
                message(c"supported_feature", c"u", &[]),
                message(c"supported_tf_named", c"u", &[]),
                message(c"supported_primaries_named", c"u", &[]),
                message(c"done", c"", &[]),
            ],
        );
        define(
            p.surface,
            vec![
                message(c"destroy", c"", &[]),
                message(
                    c"set_image_description",
                    c"ou",
                    &[p.description, ptr::null()],
                ),
                message(c"unset_image_description", c"", &[]),
            ],
            vec![],
        );
        define(
            p.feedback,
            vec![
                message(c"destroy", c"", &[]),
                message(c"get_preferred", c"n", &[p.description]),
                message(c"get_preferred_parametric", c"n", &[p.description]),
            ],
            vec![message(c"preferred_changed", c"u", &[])],
        );
        define(
            p.params,
            vec![
                message(c"create", c"n", &[p.description]),
                message(c"set_tf_named", c"u", &[]),
                message(c"set_tf_power", c"u", &[]),
                message(c"set_primaries_named", c"u", &[]),
                message(c"set_primaries", c"iiiiiiii", &[]),
                message(c"set_luminances", c"uuu", &[]),
                message(c"set_mastering_display_primaries", c"iiiiiiii", &[]),
                message(c"set_mastering_luminance", c"uu", &[]),
                message(c"set_max_cll", c"u", &[]),
                message(c"set_max_fall", c"u", &[]),
            ],
            vec![],
        );
        define(
            p.description,
            vec![
                message(c"destroy", c"", &[]),
                message(c"get_information", c"n", &[p.info]),
            ],
            vec![message(c"failed", c"us", &[]), message(c"ready", c"u", &[])],
        );
        define(
            p.info,
            vec![],
            vec![
                message(c"done", c"", &[]),
                message(c"icc_file", c"hu", &[]),
                message(c"primaries", c"iiiiiiii", &[]),
                message(c"primaries_named", c"u", &[]),
                message(c"tf_power", c"u", &[]),
                message(c"tf_named", c"u", &[]),
                message(c"luminances", c"uuu", &[]),
                message(c"target_primaries", c"iiiiiiii", &[]),
                message(c"target_luminance", c"uu", &[]),
                message(c"target_max_cll", c"u", &[]),
                message(c"target_max_fall", c"u", &[]),
            ],
        );
        Box::into_raw(Box::new(p)) as usize
    });
    unsafe { &*(address as *const Interfaces) }
}
