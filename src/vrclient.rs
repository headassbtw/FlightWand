use crate::pipe::{Hand, UI2VR, VR2UI, VRSystemFailure, VRSystemInformation};
use crate::util;
use ash::vk::{self, Handle};
use evdev::uinput::VirtualDevice;
use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, AttributeSetRef, InputEvent, KeyCode, UinputAbsSetup};
use openxr as xr;
use openxr::{Fovf, Posef};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const VIEW_COUNT: u32 = 2;
const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

const FAKE_FOV: Fovf = Fovf { angle_left: 0.0, angle_right: 0.0, angle_up: 0.0, angle_down: 0.0 };

struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
}

/// Maximum number of frames in flight
const PIPELINE_DEPTH: u32 = 2;

pub struct VRClient {}

macro_rules! ghetto_unwrap {
    ($tx: expr, $result:expr) => {
        match $result {
            core::result::Result::Ok(val) => val,
            core::result::Result::Err(err) => {
                let _ = $tx.send(VR2UI::Failure(VRSystemFailure::Generic(err)));
                return;
            }
        }
    };
}

fn bind_gamepad(axes: &[UinputAbsSetup], keys: &AttributeSetRef<KeyCode>) -> io::Result<VirtualDevice> {
    let mut device = VirtualDevice::builder()?.name("FlightWand Virtual Flight Stick");
    for axis in axes {
        device = device.with_absolute_axis(axis)?
    }

    device.with_keys(keys)?.build()
}

impl VRClient {
    pub fn run(tx: std::sync::mpsc::Sender<VR2UI>, rx: std::sync::mpsc::Receiver<UI2VR>) {
        tokio::task::spawn(async move {
            VRClient::run1(tx, rx).await;
        });
    }
    async fn run1(tx: std::sync::mpsc::Sender<VR2UI>, rx: std::sync::mpsc::Receiver<UI2VR>) {
        let mut offsets: [f32; 3] = [0.0; 3];
        let mut identity: [f32; 3] = [0.0; 3];
        identity[2] = -1.0;
        offsets[0] = -0.18;
        offsets[2] = -0.35;
        let abs_setup = AbsInfo::new(0, -100, 100, 0, 0, 200);

        let axis_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, abs_setup);
        let axis_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, abs_setup);
        let axis_z = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RUDDER, abs_setup);

        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_TRIGGER);
        keys.insert(KeyCode::BTN_TOP);
        keys.insert(KeyCode::BTN_NORTH);
        keys.insert(KeyCode::BTN_EAST);
        keys.insert(KeyCode::BTN_SOUTH);
        keys.insert(KeyCode::BTN_WEST);

        let mut device = match bind_gamepad(&[axis_x, axis_y, axis_z], &keys) {
            Ok(device) => device,
            Err(_err) => {
                let _ = tx.send(VR2UI::Failure(VRSystemFailure::VirtualGamepad));
                return;
            }
        };

        let hand;
        'wait_for_startup: loop {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    UI2VR::Start(chosen) => {
                        match chosen {
                            Hand::Left => hand = "left",
                            Hand::Right => hand = "right",
                        }
                        break 'wait_for_startup;
                    }
                    UI2VR::Shutdown => {
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Handle interrupts gracefully
        let running = Arc::new(AtomicBool::new(true));

        let entry = unsafe {
            match xr::Entry::load() {
                Ok(entry) => entry,
                Err(_e) => {
                    let _ = tx.send(VR2UI::Failure(VRSystemFailure::EntryCreation));
                    return;
                }
            }
        };

        let available_extensions = ghetto_unwrap!(tx, entry.enumerate_extensions());

        if !available_extensions.khr_vulkan_enable2 {
            let _ = tx.send(VR2UI::Failure(VRSystemFailure::VulkanUnavailable));
            return;
        }

        // OPENXR INIT

        let mut enabled_extensions = xr::ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable2 = true;
        enabled_extensions.mnd_headless = true;
        //enabled_extensions.extx_overlay = true;

        let xr_application_info = xr::ApplicationInfo {
            application_name: "FlightWand",
            application_version: 0,
            engine_name: "FlightWand",
            engine_version: 0,
            api_version: xr::Version::new(1, 0, 0),
        };

        let xr_instance = ghetto_unwrap!(tx, entry.create_instance(&xr_application_info, &enabled_extensions, &[],));

        let instance_props = ghetto_unwrap!(tx, xr_instance.properties());
        println!("loaded OpenXR runtime: {} {}", instance_props.runtime_name, instance_props.runtime_version);

        let system = ghetto_unwrap!(tx, xr_instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY));
        let system_properties = ghetto_unwrap!(tx, xr_instance.system_properties(system));

        if !system_properties.tracking_properties.orientation_tracking {
            let _ = tx.send(VR2UI::Failure(VRSystemFailure::RotationUnavailable));
            return;
        }

        // Check what blend mode is valid for this device (opaque vs transparent displays). We'll just
        // take the first one available!
        let environment_blend_mode =
            ghetto_unwrap!(tx, xr_instance.enumerate_environment_blend_modes(system, VIEW_TYPE))[0];

        // OpenXR is picky and wants to actually utilize vulkan. lol.

        let vk_target_version = vk::make_api_version(0, 1, 1, 0); // Vulkan 1.1 guarantees multiview support
        let vk_target_version_xr = xr::Version::new(1, 1, 0);

        let reqs = ghetto_unwrap!(tx, xr_instance.graphics_requirements::<xr::Vulkan>(system));

        if vk_target_version_xr < reqs.min_api_version_supported
            || vk_target_version_xr.major() > reqs.max_api_version_supported.major()
        {
            let _ = tx.send(VR2UI::Failure(VRSystemFailure::VulkanMismatch));
            return;
        }

        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            let vk_entry = match ash::Entry::load() {
                Ok(entry) => entry,
                Err(err) => {
                    let _ = tx.send(VR2UI::Failure(VRSystemFailure::VulkanLoader(err)));
                    return;
                }
            };

            let vk_app_info =
                vk::ApplicationInfo::default().application_version(0).engine_version(0).api_version(vk_target_version);

            let vk_instance = {
                let vk_instance = ghetto_unwrap!(
                    tx,
                    xr_instance.create_vulkan_instance(
                        system,
                        std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                        &vk::InstanceCreateInfo::default().application_info(&vk_app_info) as *const _ as *const _,
                    )
                )
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan instance");
                ash::Instance::load(vk_entry.static_fn(), vk::Instance::from_raw(vk_instance as _))
            };

            let vk_physical_device = vk::PhysicalDevice::from_raw(ghetto_unwrap!(
                tx,
                xr_instance.vulkan_graphics_device(system, vk_instance.handle().as_raw() as _)
            ) as _);

            let vk_device_properties = vk_instance.get_physical_device_properties(vk_physical_device);
            if vk_device_properties.api_version < vk_target_version {
                vk_instance.destroy_instance(None);
                let _ = tx.send(VR2UI::Failure(VRSystemFailure::VulkanMismatch));
                return;
            }

            let queue_family_index = vk_instance
                .get_physical_device_queue_family_properties(vk_physical_device)
                .into_iter()
                .enumerate()
                .find_map(|(queue_family_index, info)| {
                    if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                        Some(queue_family_index as u32)
                    } else {
                        None
                    }
                })
                .expect("Vulkan device has no graphics queue");

            let vk_device = {
                let vk_device = ghetto_unwrap!(
                    tx,
                    xr_instance.create_vulkan_device(
                        system,
                        std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                        vk_physical_device.as_raw() as _,
                        &vk::DeviceCreateInfo::default()
                            .queue_create_infos(&[vk::DeviceQueueCreateInfo::default()
                                .queue_family_index(queue_family_index)
                                .queue_priorities(&[1.0])])
                            .push_next(&mut vk::PhysicalDeviceMultiviewFeatures {
                                multiview: vk::TRUE,
                                ..Default::default()
                            }) as *const _ as *const _,
                    )
                )
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan device");

                ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
            };

            let queue = vk_device.get_device_queue(queue_family_index, 0);

            let (session, mut frame_wait, mut frame_stream) = ghetto_unwrap!(
                tx,
                xr_instance.create_session::<xr::Vulkan>(system, &xr::vulkan::SessionCreateInfo {
                    instance: vk_instance.handle().as_raw() as _,
                    physical_device: vk_physical_device.as_raw() as _,
                    device: vk_device.handle().as_raw() as _,
                    queue_family_index,
                    queue_index: 0,
                },)
            );

            let action_set = ghetto_unwrap!(tx, xr_instance.create_action_set("input", "input pose information", 0));

            let right_action = ghetto_unwrap!(tx, action_set.create_action::<xr::Posef>("hand", "Controller", &[]));

            let trackpad_x = ghetto_unwrap!(tx, action_set.create_action::<f32>("trackpad_x", "Trackpad X", &[]));
            let trackpad_y = ghetto_unwrap!(tx, action_set.create_action::<f32>("trackpad_y", "Trackpad Y", &[]));
            let trackpad_click =
                ghetto_unwrap!(tx, action_set.create_action::<bool>("trackpad_click", "Trackpad Click", &[]));

            let trigger = ghetto_unwrap!(tx, action_set.create_action::<f32>("trigger", "Trigger", &[]));

            // BINDINGS
            ghetto_unwrap!(
                tx,
                xr_instance.suggest_interaction_profile_bindings(
                    ghetto_unwrap!(tx, xr_instance.string_to_path("/interaction_profiles/htc/vive_controller")),
                    &[
                        xr::Binding::new(
                            &right_action,
                            ghetto_unwrap!(
                                tx,
                                xr_instance.string_to_path(&format!("/user/hand/{hand}/input/aim/pose"))
                            ),
                        ),
                        xr::Binding::new(
                            &trackpad_x,
                            ghetto_unwrap!(
                                tx,
                                xr_instance.string_to_path(&format!("/user/hand/{hand}/input/trackpad/x"))
                            ),
                        ),
                        xr::Binding::new(
                            &trackpad_y,
                            ghetto_unwrap!(
                                tx,
                                xr_instance.string_to_path(&format!("/user/hand/{hand}/input/trackpad/y"))
                            ),
                        ),
                        xr::Binding::new(
                            &trackpad_click,
                            ghetto_unwrap!(
                                tx,
                                xr_instance.string_to_path(&format!("/user/hand/{hand}/input/trackpad/click"))
                            ),
                        ),
                        xr::Binding::new(
                            &trigger,
                            ghetto_unwrap!(
                                tx,
                                xr_instance.string_to_path(&format!("/user/hand/{hand}/input/trigger/value"))
                            ),
                        ),
                    ],
                )
            );

            ghetto_unwrap!(tx, session.attach_action_sets(&[&action_set]));

            let right_space =
                ghetto_unwrap!(tx, right_action.create_space(session.clone(), xr::Path::NULL, xr::Posef::IDENTITY));

            let stage =
                ghetto_unwrap!(tx, session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY));

            let cmd_pool = vk_device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default().queue_family_index(queue_family_index).flags(
                        vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER | vk::CommandPoolCreateFlags::TRANSIENT,
                    ),
                    None,
                )
                .unwrap();
            let cmds = vk_device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(cmd_pool)
                        .command_buffer_count(PIPELINE_DEPTH),
                )
                .unwrap();
            let fences = (0..PIPELINE_DEPTH)
                .map(|_| {
                    vk_device
                        .create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None)
                        .unwrap()
                })
                .collect::<Vec<_>>();

            // tell the frontend we're good!
            let _ = tx.send(VR2UI::Running(VRSystemInformation { system_properties }));

            // Main loop
            let mut swapchain = None;
            let mut event_storage = xr::EventDataBuffer::new();
            let mut session_running = false;
            // Index of the current frame, wrapped by PIPELINE_DEPTH. Not to be confused with the
            // swapchain image index.
            let mut frame = 0;
            'main_loop: loop {
                if !running.load(Ordering::Relaxed) {
                    println!("requesting exit");
                    // The OpenXR runtime may want to perform a smooth transition between scenes, so we
                    // can't necessarily exit instantly. Instead, we must notify the runtime of our
                    // intent and wait for it to tell us when we're actually done.
                    match session.request_exit() {
                        Ok(()) => {}
                        Err(xr::sys::Result::ERROR_SESSION_NOT_RUNNING) => break,
                        Err(e) => panic!("{}", e),
                    }
                }

                while let Some(event) = ghetto_unwrap!(tx, xr_instance.poll_event(&mut event_storage)) {
                    use xr::Event::*;
                    match event {
                        SessionStateChanged(e) => {
                            // Session state change is where we can begin and end sessions, as well as
                            // find quit messages!
                            println!("entered state {:?}", e.state());
                            match e.state() {
                                xr::SessionState::READY => {
                                    ghetto_unwrap!(tx, session.begin(VIEW_TYPE));
                                    session_running = true;
                                }
                                xr::SessionState::STOPPING => {
                                    ghetto_unwrap!(tx, session.end());
                                    session_running = false;
                                }
                                xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                                    break 'main_loop;
                                }
                                _ => {}
                            }
                        }
                        InstanceLossPending(_) => {
                            break 'main_loop;
                        }
                        EventsLost(e) => {
                            println!("lost {} events", e.lost_event_count());
                        }
                        _ => {}
                    }
                }

                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        UI2VR::Shutdown => break 'main_loop,
                        UI2VR::UpdateOffsets(new_offsets) => {
                            offsets = new_offsets;
                        }
                        UI2VR::UpdateIdentity(new_id) => {
                            identity = new_id;
                        }
                        _ => {}
                    }
                }

                if !session_running {
                    // Don't grind up the CPU
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }

                let xr_frame_state = ghetto_unwrap!(tx, frame_wait.wait());
                ghetto_unwrap!(tx, frame_stream.begin());

                if !xr_frame_state.should_render {
                    ghetto_unwrap!(
                        tx,
                        frame_stream.end(xr_frame_state.predicted_display_time, environment_blend_mode, &[],)
                    );
                    continue;
                }

                let swapchain = swapchain.get_or_insert_with(|| {
                    let handle = session
                        .create_swapchain(&xr::SwapchainCreateInfo {
                            create_flags: xr::SwapchainCreateFlags::EMPTY,
                            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
                            format: COLOR_FORMAT.as_raw() as _,
                            sample_count: 1,
                            width: 1,
                            height: 1,
                            face_count: 1,
                            array_size: VIEW_COUNT,
                            mip_count: 1,
                        })
                        .unwrap();

                    Swapchain { handle }
                });

                // frame cleanup
                let _image_index = swapchain.handle.acquire_image().unwrap();
                vk_device.wait_for_fences(&[fences[frame]], true, u64::MAX).unwrap();
                vk_device.reset_fences(&[fences[frame]]).unwrap();

                let cmd = cmds[frame];
                vk_device
                    .begin_command_buffer(
                        cmd,
                        &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                    )
                    .unwrap();

                vk_device.end_command_buffer(cmd).unwrap();

                ghetto_unwrap!(tx, session.sync_actions(&[(&action_set).into()]));

                let pose = ghetto_unwrap!(tx, right_space.locate(&stage, xr_frame_state.predicted_display_time));

                if ghetto_unwrap!(tx, right_action.is_active(&session, xr::Path::NULL)) {
                    // don't unwrap because sometimes the UI can shut down in the middle of this function
                    let _ = tx.send(VR2UI::RotationUpdate(pose.pose.orientation));
                }

                let trigger = ghetto_unwrap!(tx, trigger.state(&session, xr::Path::NULL));
                let trackpad_x = ghetto_unwrap!(tx, trackpad_x.state(&session, xr::Path::NULL));
                let trackpad_y = ghetto_unwrap!(tx, trackpad_y.state(&session, xr::Path::NULL));
                let trackpad_click = ghetto_unwrap!(tx, trackpad_click.state(&session, xr::Path::NULL));

                if trackpad_x.is_active && trackpad_y.is_active && trackpad_click.is_active {
                    let ang = f32::atan2(trackpad_x.current_state, trackpad_y.current_state) / std::f32::consts::PI;
                    let distance = f32::sqrt(
                        trackpad_x.current_state * trackpad_x.current_state
                            + trackpad_y.current_state * trackpad_y.current_state,
                    );
                    let act = distance > 0.35 && trackpad_click.current_state;

                    let event_north = InputEvent::new(
                        1,
                        KeyCode::BTN_NORTH.0,
                        if (ang < 0.25 && ang > -0.25) && act { 1 } else { 0 },
                    );
                    let event_east =
                        InputEvent::new(1, KeyCode::BTN_EAST.0, if (ang < 0.75 && ang > 0.25) && act { 1 } else { 0 });
                    let event_west = InputEvent::new(
                        1,
                        KeyCode::BTN_WEST.0,
                        if (ang < -0.25 && ang > -0.75) && act { 1 } else { 0 },
                    );
                    let event_south = InputEvent::new(
                        1,
                        KeyCode::BTN_SOUTH.0,
                        if (ang < -0.75 || ang > 0.75) && act { 1 } else { 0 },
                    );
                    let event_trigger =
                        InputEvent::new(1, KeyCode::BTN_TRIGGER.0, if trigger.current_state > 0.6 { 1 } else { 0 });
                    device.emit(&[event_north, event_east, event_west, event_south, event_trigger]).unwrap();

                    let mut rot = util::modifier(&[
                        pose.pose.orientation.x,
                        pose.pose.orientation.y,
                        pose.pose.orientation.z,
                        pose.pose.orientation.w,
                    ]);

                    // rustfmt refuses to let me just "if bigger then big" so i have to set it! thanks!
                    rot[0] = if rot[0] > 1.0 { 1.0 } else { rot[0] };
                    rot[0] = if rot[0] < -1.0 { -1.0 } else { rot[0] };
                    rot[2] = if rot[2] > 1.0 { 1.0 } else { rot[2] };
                    rot[2] = if rot[2] < -1.0 { -1.0 } else { rot[2] };

                    let event_x = InputEvent::new(3, AbsoluteAxisCode::ABS_X.0, (rot[0] * 100.0) as i32);
                    let event_y = InputEvent::new(3, AbsoluteAxisCode::ABS_Y.0, (rot[2] * 100.0) as i32);

                    device.emit(&[event_x, event_y]).unwrap();
                }

                // Wait until the image is available to render to before beginning work on the GPU. The
                // compositor could still be reading from it.
                swapchain.handle.wait_image(xr::Duration::INFINITE).unwrap();

                // Submit commands to the GPU, then tell OpenXR we're done with our part.
                vk_device
                    .queue_submit(queue, &[vk::SubmitInfo::default().command_buffers(&[cmd])], fences[frame])
                    .unwrap();
                swapchain.handle.release_image().unwrap();

                // Tell OpenXR what to present for this frame
                let rect =
                    xr::Rect2Di { offset: xr::Offset2Di { x: 0, y: 0 }, extent: xr::Extent2Di { width: 1, height: 1 } };
                ghetto_unwrap!(
                    tx,
                    frame_stream.end(xr_frame_state.predicted_display_time, environment_blend_mode, &[
                        &xr::CompositionLayerProjection::new().space(&stage).views(&[
                            xr::CompositionLayerProjectionView::new().pose(Posef::IDENTITY).fov(FAKE_FOV).sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(0)
                                    .image_rect(rect),
                            ),
                            xr::CompositionLayerProjectionView::new().pose(Posef::IDENTITY).fov(FAKE_FOV).sub_image(
                                xr::SwapchainSubImage::new()
                                    .swapchain(&swapchain.handle)
                                    .image_array_index(1)
                                    .image_rect(rect),
                            ),
                        ]),
                    ],)
                );
                frame = (frame + 1) % PIPELINE_DEPTH as usize;
            } // 'main_loop

            // OpenXR MUST be allowed to clean up before we destroy Vulkan resources it could touch, so
            // first we must drop all its handles.
            drop((session, frame_wait, frame_stream, stage, action_set, right_space, right_action));
        }

        println!("exiting cleanly");
    }
}
