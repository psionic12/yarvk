use std::collections::HashMap;
use std::ffi::CStr;
use std::io::Cursor;
use std::sync::Arc;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use yarvk::barrier::ImageMemoryBarrier;
use yarvk::buffer::Buffer;
use yarvk::command::command_buffer::Level::PRIMARY;
use yarvk::command::command_pool::{CommandPool, CommandPoolCreateFlags};
use yarvk::debug_utils_messenger::DebugUtilsMessengerCreateInfoEXT;
use yarvk::descriptor_pool::{
    DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, WriteDescriptorSet, DESCRIPTOR_INFO_TYPE_BUFFER,
    DESCRIPTOR_INFO_TYPE_IMAGE,
};
use yarvk::device::{Device, DeviceQueueCreateInfo};

use yarvk::device_memory::DeviceMemory;
use yarvk::entry::Entry;
use yarvk::extensions::{
    DeviceExtensionType, PhysicalDeviceExtensionType, PhysicalInstanceExtensionType,
};
use yarvk::fence::Fence;
use yarvk::frame_buffer::Framebuffer;
use yarvk::image::image_subresource_range::ImageSubresourceRange;
use yarvk::image::image_view::{ImageView, ImageViewType};
use yarvk::image::Image;
use yarvk::image::State::Bound;
use yarvk::instance::{ApplicationInfo, Instance};
use yarvk::physical_device::memory_properties::{MemoryType, PhysicalDeviceMemoryProperties};
use yarvk::physical_device::SharingMode;
use yarvk::pipeline::color_blend_state::{
    BlendFactor, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
};
use yarvk::pipeline::depth_stencil_state::PipelineDepthStencilStateCreateInfo;
use yarvk::pipeline::input_assembly_state::{
    PipelineInputAssemblyStateCreateInfo, PrimitiveTopology,
};
use yarvk::pipeline::multisample_state::PipelineMultisampleStateCreateInfo;
use yarvk::pipeline::pipeline_stage_flags::PipelineStageFlags;
use yarvk::pipeline::rasterization_state::{PipelineRasterizationStateCreateInfo, PolygonMode};
use yarvk::pipeline::shader_stage::{PipelineShaderStageCreateInfo, ShaderStageFlags};
use yarvk::pipeline::vertex_input_state::{
    PipelineVertexInputStateCreateInfo, VertexInputAttributeDescription,
    VertexInputBindingDescription,
};
use yarvk::pipeline::viewport_state::PipelineViewportStateCreateInfo;
use yarvk::pipeline::{Pipeline, PipelineLayout};
use yarvk::queue::SubmitInfo;
use yarvk::render_pass::attachment::{AttachmentDescription, AttachmentReference};
use yarvk::render_pass::render_pass_begin_info::RenderPassBeginInfo;
use yarvk::render_pass::subpass::{SubpassDependency, SubpassDescription};
use yarvk::render_pass::RenderPass;
use yarvk::sampler::Sampler;
use yarvk::semaphore::Semaphore;
use yarvk::shader_module::ShaderModule;
use yarvk::surface::Surface;
use yarvk::swapchain::{PresentInfo, Swapchain};
use yarvk::window::enumerate_required_extensions;
use yarvk::{read_spv};
use yarvk::{
    AccessFlags, AttachmentLoadOp, AttachmentStoreOp, BlendOp, BorderColor, BufferImageCopy,
    BufferUsageFlags, ClearColorValue, ClearDepthStencilValue, ClearValue, ColorComponentFlags,
    CommandBufferUsageFlags, CompareOp, ComponentMapping, ComponentSwizzle, CompositeAlphaFlagsKHR,
    DebugUtilsMessageSeverityFlagsEXT, DependencyFlags, DescriptorPoolSize, DescriptorType,
    Extent2D, Extent3D, Filter, Format, FrontFace, ImageAspectFlags, ImageLayout,
    ImageSubresourceLayers, ImageTiling, ImageType, ImageUsageFlags, IndexType,
    MemoryPropertyFlags, MemoryRequirements, PipelineBindPoint, PresentModeKHR, QueueFlags, Rect2D,
    SampleCountFlags, SamplerAddressMode, SamplerMipmapMode, StencilOp, StencilOpState,
    SubpassContents, SurfaceTransformFlagsKHR, VertexInputRate, Viewport, SUBPASS_EXTERNAL,
};
#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = std::mem::zeroed();
            (&b.$field as *const _ as isize) - (&b as *const _ as isize)
        }
    }};
}

#[derive(Clone, Debug, Copy)]
struct Vertex {
    pos: [f32; 4],
    uv: [f32; 2],
}

#[derive(Clone, Debug, Copy)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub _pad: f32,
}

pub fn find_memory_type_index(
    memory_req: &MemoryRequirements,
    memory_prop: &PhysicalDeviceMemoryProperties,
    flags: MemoryPropertyFlags,
) -> Option<MemoryType> {
    memory_prop
        .memory_types
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(_index, memory_type)| memory_type.clone())
}

fn main() {
    let window_width = 1920;
    let window_height = 1080;
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("yarvk_example")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(1920),
            f64::from(1080),
        ))
        .build(&event_loop)
        .unwrap();

    let entry = Entry::load().unwrap();
    let surface_extensions = enumerate_required_extensions(&window).unwrap();

    let layer = unsafe { CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0") };
    let debug_utils_messenger_callback = DebugUtilsMessengerCreateInfoEXT::builder()
        .callback(|message_severity, message_type, p_callback_data| {
            let message_id_number = p_callback_data.message_id_number;
            let message_id_name = p_callback_data.p_message_id_name;
            let message = p_callback_data.p_message;
            println!(
                "{:?}:\n{:?} [{} ({})] : {}\n",
                message_severity,
                message_type,
                message_id_name,
                message_id_number.to_string(),
                message,
            );
        })
        .severity(
            // DebugUtilsMessageSeverityFlagsEXT::VERBOSE | DebugUtilsMessageSeverityFlagsEXT::INFO |
            DebugUtilsMessageSeverityFlagsEXT::WARNING | DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .build();
    let application_info = ApplicationInfo::builder()
        .engine_name("yarvk_example")
        .build();
    let mut instance_builder = Instance::builder(entry.clone())
        .application_info(application_info)
        .add_layer(layer)
        .debug_utils_messenger_exts(vec![debug_utils_messenger_callback]);
    for ext in surface_extensions {
        instance_builder = instance_builder.add_extension(&ext);
    }
    let instance = instance_builder.build().unwrap();
    let khr_surface_ext = instance
        .get_extension::<{ PhysicalInstanceExtensionType::KhrSurface }>()
        .unwrap();
    let pdevices = instance.enumerate_physical_devices().unwrap();
    let (pdevice, queue_family, surface) = pdevices
        .iter()
        .filter_map(|pdevice| {
            pdevice
                .get_physical_device_queue_family_properties()
                .into_iter()
                .filter_map(|queue_family_properties| {
                    if let Some(surface) = Surface::get_physical_device_surface_support(
                        khr_surface_ext.clone(),
                        &window,
                        &queue_family_properties,
                    )
                    .unwrap()
                    {
                        if queue_family_properties
                            .property
                            .queue_flags
                            .contains(QueueFlags::GRAPHICS)
                        {
                            return Some((pdevice, queue_family_properties, surface));
                        }
                    }
                    None
                })
                .next()
        })
        .next()
        .expect("Couldn't find suitable device.");
    // let portable_property = pdevice.get_physical_device_properties2::<PhysicalDevicePortabilitySubsetPropertiesKHR>();
    // println!("min_vertex_input_binding_stride_alignment: {}", portable_property.min_vertex_input_binding_stride_alignment);
    // let prop2_ext = instance.get_extension::<{ PhysicalInstanceExtensionType::KhrGetPhysicalDeviceProperties2 }>().unwrap();
    let surface_ext = instance
        .get_extension::<{ PhysicalInstanceExtensionType::KhrSurface }>()
        .unwrap();
    let queue_create_info = DeviceQueueCreateInfo::builder(queue_family.clone())
        .add_priority(0.9)
        .build();
    let (device, mut queues) = Device::builder(pdevice.clone())
        .add_queue_info(queue_create_info)
        .add_extension(&DeviceExtensionType::KhrSwapchain(surface_ext))
        // .add_feature(PhysicalDeviceFeatures::LogicOp.into())
        // .add_feature(DevicePortabilitySubsetFeaturesKHR::ImageViewFormatSwizzle().into());
        .build()
        .unwrap();
    let swapchian_extension = device
        .get_extension::<{ PhysicalDeviceExtensionType::KhrSwapchain }>()
        .unwrap();
    let mut present_queue = queues.get_mut(&queue_family).unwrap().pop().unwrap();
    let surface_format = surface.get_physical_device_surface_formats()[0];
    let surface_capabilities = surface.get_physical_device_surface_capabilities();
    let mut desired_image_count = surface_capabilities.min_image_count + 1;
    if surface_capabilities.max_image_count > 0
        && desired_image_count > surface_capabilities.max_image_count
    {
        desired_image_count = surface_capabilities.max_image_count;
    }
    let surface_resolution = match surface_capabilities.current_extent.width {
        u32::MAX => Extent2D {
            width: window_width,
            height: window_height,
        },
        _ => surface_capabilities.current_extent,
    };
    let pre_transform = if surface_capabilities
        .supported_transforms
        .contains(SurfaceTransformFlagsKHR::IDENTITY)
    {
        SurfaceTransformFlagsKHR::IDENTITY
    } else {
        surface_capabilities.current_transform
    };
    let present_modes = surface.get_physical_device_surface_present_modes();
    let present_mode = present_modes
        .iter()
        .cloned()
        .find(|&mode| mode == PresentModeKHR::MAILBOX)
        .unwrap_or(PresentModeKHR::FIFO);
    let swapchain = Swapchain::builder(surface.clone(), swapchian_extension.clone())
        .min_image_count(desired_image_count)
        .image_color_space(surface_format.color_space)
        .image_format(surface_format.format)
        .image_extent(surface_resolution)
        .image_sharing_mode(SharingMode::EXCLUSIVE)
        .pre_transform(pre_transform)
        .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped()
        .image_array_layers(1)
        .build()
        .unwrap();
    let pool = CommandPool::builder(queue_family, device.clone())
        // do not need, yarvk enable reset feature by default
        .add_flag(CommandPoolCreateFlags::ResetCommandBuffer)
        .build()
        .unwrap();
    let mut command_buffers = pool.allocate_command_buffers::<{ PRIMARY }>(2).unwrap();
    let setup_command_buffer = command_buffers.pop().unwrap();
    let mut draw_command_buffer = command_buffers.pop();
    let present_images = swapchain.get_swapchain_images();
    let present_image_views: Vec<Arc<ImageView>> = present_images
        .iter()
        .map(|image| {
            ImageView::builder(image.clone())
                .view_type(ImageViewType::Type2d)
                .format(surface_format.format)
                .components(ComponentMapping {
                    r: ComponentSwizzle::R,
                    g: ComponentSwizzle::G,
                    b: ComponentSwizzle::B,
                    a: ComponentSwizzle::A,
                })
                .subresource_range(
                    ImageSubresourceRange::builder()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                )
                .build()
                .unwrap()
        })
        .collect();
    let device_memory_properties = pdevice.memory_properties();
    let depth_image = Image::builder(device.clone())
        .image_type(ImageType::TYPE_2D)
        .format(Format::D16_UNORM)
        .extent(surface_resolution.into())
        .mip_levels(1)
        .array_layers(1)
        .samples(SampleCountFlags::TYPE_1)
        .tiling(ImageTiling::OPTIMAL)
        .usage(ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();

    let depth_image_memory_req = depth_image.get_image_memory_requirements();
    let depth_image_memory = find_memory_type_index(
        &depth_image_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .expect("Unable to find suitable memory index for depth image.");
    let depth_image_memory = DeviceMemory::builder(depth_image_memory, device.clone())
        .allocation_size(depth_image_memory_req.size)
        .build()
        .unwrap();
    let depth_image = depth_image
        .bind_memory(&depth_image_memory, 0)
        .expect("Unable to bind depth image memory");

    let draw_commands_reuse_fence = Fence::new(device.clone()).unwrap();
    let mut draw_commands_reuse_fence = Some(draw_commands_reuse_fence);
    let setup_commands_reuse_fence = Fence::new(device.clone()).unwrap();

    let command_buffer = setup_command_buffer
        .record(CommandBufferUsageFlags::ONE_TIME_SUBMIT, |command_buffer| {
            command_buffer.cmd_pipeline_barrier(
                &[PipelineStageFlags::BottomOfPipe],
                &[PipelineStageFlags::LateFragmentTests],
                DependencyFlags::empty(),
                &[],
                &[],
                &[ImageMemoryBarrier::builder(depth_image.clone())
                    .dst_access_mask(
                        AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .new_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .old_layout(ImageLayout::UNDEFINED)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .aspect_mask(ImageAspectFlags::DEPTH)
                            .layer_count(1)
                            .level_count(1)
                            .build(),
                    )
                    .build()],
            );
        })
        .unwrap();
    let mut submit_info = SubmitInfo::new();
    submit_info.add_command_buffer(command_buffer);
    let fence = present_queue
        .submit(setup_commands_reuse_fence, vec![submit_info])
        .expect("queue submit failed.");
    let (fence, mut infos) = fence.wait().unwrap();
    let setup_commands_reuse_fence = fence.reset().unwrap();
    let mut submit_info = infos.pop().unwrap();
    let setup_command_buffer = submit_info
        .take_invalid_buffers()
        .pop()
        .unwrap()
        .reset()
        .unwrap();

    let depth_image_view = ImageView::builder(depth_image.clone())
        .subresource_range(
            ImageSubresourceRange::builder()
                .aspect_mask(ImageAspectFlags::DEPTH)
                .level_count(1)
                .layer_count(1)
                .build(),
        )
        .format(depth_image.image_create_info.format)
        .view_type(ImageViewType::Type2d)
        .build()
        .unwrap();

    let mut renderpass_builder = RenderPass::builder(device.clone());
    let renderpass_attachment0 = renderpass_builder.add_attachment(
        AttachmentDescription::builder()
            .format(surface_format.format)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .store_op(AttachmentStoreOp::STORE)
            .final_layout(ImageLayout::PRESENT_SRC_KHR)
            .build(),
    );
    let renderpass_attachment1 = renderpass_builder.add_attachment(
        AttachmentDescription::builder()
            .format(Format::D16_UNORM)
            .samples(SampleCountFlags::TYPE_1)
            .load_op(AttachmentLoadOp::CLEAR)
            .initial_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            .build(),
    );
    let subpass = SubpassDescription::builder()
        .add_color_attachment(
            AttachmentReference::builder()
                .attachment_index(renderpass_attachment0)
                .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .build(),
        )
        .depth_stencil_attachment(
            AttachmentReference::builder()
                .attachment_index(renderpass_attachment1)
                .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .build(),
        )
        .build();
    let subpass_id0 = renderpass_builder.add_subpass(subpass);
    renderpass_builder.add_dependency(
        SubpassDependency::builder()
            .src_subpass(SUBPASS_EXTERNAL)
            .add_src_stage_mask(PipelineStageFlags::ColorAttachmentOutput)
            .add_dst_stage_mask(PipelineStageFlags::ColorAttachmentOutput)
            .dst_access_mask(
                AccessFlags::COLOR_ATTACHMENT_READ | AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .build(),
    );
    let renderpass = renderpass_builder.build().unwrap();
    let framebuffers: HashMap<Arc<Image<{ Bound }>>, Arc<Framebuffer>> = present_image_views
        .iter()
        .map(|present_image_view| {
            let framebuffer = Framebuffer::builder(renderpass.clone())
                .add_attachment(renderpass_attachment0, present_image_view.clone())
                .add_attachment(renderpass_attachment1, depth_image_view.clone())
                .width(surface_resolution.width)
                .height(surface_resolution.height)
                .layers(1)
                .build(device.clone())
                .unwrap();
            (present_image_view.image.clone(), framebuffer)
        })
        .collect();
    let index_buffer_data = [0u32, 1, 2, 2, 3, 0];
    let index_buffer = Buffer::builder(device.clone())
        .size(std::mem::size_of_val(&index_buffer_data) as u64)
        .usage(BufferUsageFlags::INDEX_BUFFER)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();
    let index_buffer_memory_req = index_buffer.get_buffer_memory_requirements();
    let index_buffer_memory_index = find_memory_type_index(
        &index_buffer_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )
    .expect("Unable to find suitable memorytype for the index buffer.");
    let mut index_buffer_memory = DeviceMemory::builder(index_buffer_memory_index, device.clone())
        .allocation_size(index_buffer_memory_req.size)
        .build()
        .unwrap();
    index_buffer_memory
        .map_memory(0, index_buffer_memory_req.size, |mut_slice| {
            mut_slice[0..std::mem::size_of_val(&index_buffer_data)].copy_from_slice(unsafe {
                std::slice::from_raw_parts(
                    index_buffer_data.as_ptr() as *const u8,
                    std::mem::size_of_val(&index_buffer_data),
                )
            });
        })
        .unwrap();
    let index_buffer = index_buffer.bind_memory(&index_buffer_memory, 0).unwrap();

    let vertices = [
        Vertex {
            pos: [-1.0, -1.0, 0.0, 1.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            pos: [-1.0, 1.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
        Vertex {
            pos: [1.0, 1.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            pos: [1.0, -1.0, 0.0, 1.0],
            uv: [1.0, 0.0],
        },
    ];

    let vertex_input_buffer = Buffer::builder(device.clone())
        .size(std::mem::size_of_val(&vertices) as _)
        .usage(BufferUsageFlags::VERTEX_BUFFER)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();

    let vertex_input_buffer_memory_req = vertex_input_buffer.get_buffer_memory_requirements();

    let vertex_input_buffer_memory_index = find_memory_type_index(
        &vertex_input_buffer_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )
    .expect("Unable to find suitable memorytype for the vertex buffer.");

    let mut vertex_input_buffer_memory =
        DeviceMemory::builder(vertex_input_buffer_memory_index, device.clone())
            .allocation_size(vertex_input_buffer_memory_req.size)
            .build()
            .unwrap();

    vertex_input_buffer_memory
        .map_memory(0, vertex_input_buffer_memory_req.size, |mut_slice| {
            // TODO check alignment
            mut_slice[0..std::mem::size_of_val(&vertices)].copy_from_slice(unsafe {
                std::slice::from_raw_parts(
                    vertices.as_ptr() as *const u8,
                    std::mem::size_of_val(&vertices),
                )
            });
        })
        .unwrap();
    let vertex_input_buffer = vertex_input_buffer
        .bind_memory(&vertex_input_buffer_memory, 0)
        .unwrap();

    let uniform_color_buffer_data = Vector3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        _pad: 0.0,
    };

    let uniform_color_buffer = Buffer::builder(device.clone())
        .size(std::mem::size_of_val(&uniform_color_buffer_data) as u64)
        .usage(BufferUsageFlags::UNIFORM_BUFFER)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();
    let uniform_color_buffer_memory_req = uniform_color_buffer.get_buffer_memory_requirements();
    let uniform_color_buffer_memory_index = find_memory_type_index(
        &uniform_color_buffer_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )
    .expect("Unable to find suitable memorytype for the vertex buffer.");
    let mut uniform_color_buffer_memory =
        DeviceMemory::builder(uniform_color_buffer_memory_index, device.clone())
            .allocation_size(uniform_color_buffer_memory_req.size)
            .build()
            .unwrap();

    uniform_color_buffer_memory
        .map_memory(0, uniform_color_buffer_memory_req.size, |mut_slice| {
            mut_slice[0..std::mem::size_of_val(&uniform_color_buffer_data)].copy_from_slice(
                unsafe {
                    std::slice::from_raw_parts(
                        &uniform_color_buffer_data as *const _ as *const u8,
                        std::mem::size_of_val(&uniform_color_buffer_data),
                    )
                },
            );
        })
        .unwrap();

    let uniform_color_buffer = uniform_color_buffer
        .bind_memory(&uniform_color_buffer_memory, 0)
        .unwrap();

    let image = image::load_from_memory(include_bytes!("rust.png"))
        .unwrap()
        .to_rgba8();
    let (width, height) = image.dimensions();
    let image_extent = Extent2D { width, height };
    let image_data = image.into_raw();

    let image_buffer = Buffer::builder(device.clone())
        .size(image_data.len() as _)
        .usage(BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();
    let image_buffer_memory_req = image_buffer.get_buffer_memory_requirements();
    let image_buffer_memory_index = find_memory_type_index(
        &image_buffer_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )
    .expect("Unable to find suitable memorytype for the vertex buffer.");

    let mut image_buffer_memory = DeviceMemory::builder(image_buffer_memory_index, device.clone())
        .allocation_size(image_buffer_memory_req.size)
        .build()
        .unwrap();
    image_buffer_memory
        .map_memory(0, image_buffer_memory_req.size, |mut_slice| {
            mut_slice[0..image_data.len()].copy_from_slice(image_data.as_slice());
        })
        .unwrap();
    let image_buffer = image_buffer.bind_memory(&image_buffer_memory, 0).unwrap();

    let texture_image = Image::builder(device.clone())
        .image_type(ImageType::TYPE_2D)
        .format(Format::R8G8B8A8_UNORM)
        .extent(image_extent.into())
        .mip_levels(1)
        .array_layers(1)
        .samples(SampleCountFlags::TYPE_1)
        .tiling(ImageTiling::OPTIMAL)
        .usage(ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .build()
        .unwrap();
    let texture_memory_req = texture_image.get_image_memory_requirements();
    let texture_memory_index = find_memory_type_index(
        &texture_memory_req,
        &device_memory_properties,
        MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .expect("Unable to find suitable memory index for depth image.");

    let texture_memory = DeviceMemory::builder(texture_memory_index, device.clone())
        .allocation_size(texture_memory_req.size)
        .build()
        .unwrap();
    let texture_image = texture_image
        .bind_memory(&texture_memory, 0)
        .expect("Unable to bind depth image memory");

    let command_buffer = setup_command_buffer
        .record(CommandBufferUsageFlags::ONE_TIME_SUBMIT, |command_buffer| {
            let texture_barrier = ImageMemoryBarrier::builder(texture_image.clone())
                .dst_access_mask(AccessFlags::TRANSFER_WRITE)
                .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .subresource_range(
                    ImageSubresourceRange::builder()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .build();
            let mut image_barriers = Vec::with_capacity(1);
            image_barriers.push(texture_barrier);
            command_buffer.cmd_pipeline_barrier(
                &[PipelineStageFlags::BottomOfPipe],
                &[PipelineStageFlags::Transfer],
                DependencyFlags::empty(),
                &[],
                &[],
                &image_barriers,
            );
            let buffer_copy_regions = BufferImageCopy::builder()
                .image_subresource(
                    ImageSubresourceLayers::builder()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .layer_count(1)
                        .build(),
                )
                .image_extent(Extent3D {
                    width,
                    height,
                    depth: 1,
                });

            command_buffer.cmd_copy_buffer_to_image(
                image_buffer.clone(),
                texture_image.clone(),
                ImageLayout::TRANSFER_DST_OPTIMAL,
                &[buffer_copy_regions.build()],
            );
            let texture_barrier_end = ImageMemoryBarrier::builder(texture_image.clone())
                .src_access_mask(AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(AccessFlags::SHADER_READ)
                .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .subresource_range(
                    ImageSubresourceRange::builder()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .build();
            let mut image_barriers = Vec::with_capacity(1);
            image_barriers.push(texture_barrier_end);
            command_buffer.cmd_pipeline_barrier(
                &[PipelineStageFlags::Transfer],
                &[PipelineStageFlags::FragmentShader],
                DependencyFlags::empty(),
                &[],
                &[],
                &image_barriers,
            );
        })
        .unwrap();
    let mut submit_info = SubmitInfo::new();
    submit_info.add_command_buffer(command_buffer);
    let fence = present_queue
        .submit(setup_commands_reuse_fence, vec![submit_info])
        .expect("queue submit failed.");
    fence.wait().unwrap();

    let sampler = Sampler::builder(device.clone())
        .mag_filter(Filter::LINEAR)
        .min_filter(Filter::LINEAR)
        .mipmap_mode(SamplerMipmapMode::LINEAR)
        .address_mode_u(SamplerAddressMode::MIRRORED_REPEAT)
        .address_mode_v(SamplerAddressMode::MIRRORED_REPEAT)
        .address_mode_w(SamplerAddressMode::MIRRORED_REPEAT)
        // .max_anisotropy(1.0)
        .border_color(BorderColor::FLOAT_OPAQUE_WHITE)
        .compare_op(CompareOp::NEVER)
        .build()
        .unwrap();

    let tex_image_view = ImageView::builder(texture_image.clone())
        .view_type(ImageViewType::Type2d)
        .format(texture_image.image_create_info.format)
        .components(ComponentMapping {
            r: ComponentSwizzle::R,
            g: ComponentSwizzle::G,
            b: ComponentSwizzle::B,
            a: ComponentSwizzle::A,
        })
        .subresource_range(
            ImageSubresourceRange::builder()
                .aspect_mask(ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1)
                .build(),
        )
        .build()
        .unwrap();

    let descriptor_pool = DescriptorPool::builder(device.clone())
        .max_sets(1)
        .add_descriptor_pool_size(DescriptorPoolSize {
            ty: DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
        })
        .add_descriptor_pool_size(DescriptorPoolSize {
            ty: DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
        })
        .build()
        .unwrap();

    let desc_set_layout = DescriptorSetLayout::builder(device.clone())
        .add_binding(
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .add_stage_flag(ShaderStageFlags::Fragment)
                .build(),
        )
        .add_binding(
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .add_stage_flag(ShaderStageFlags::Fragment)
                .build(),
        )
        .build()
        .unwrap();

    let descriptor_sets = DescriptorSet::builder(descriptor_pool.clone())
        .add_set_layout(desc_set_layout.clone())
        .build()
        .unwrap();
    let uniform_color_buffer_descriptor = DescriptorBufferInfo {
        buffer: uniform_color_buffer.clone(),
        offset: 0,
        range: std::mem::size_of_val(&uniform_color_buffer_data) as u64,
    };

    let tex_descriptor = DescriptorImageInfo {
        image_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        image_view: tex_image_view.clone(),
        sampler: sampler.clone(),
    };
    let mut write_desc_sets = Vec::with_capacity(2);
    write_desc_sets.push(
        WriteDescriptorSet::builder::<DESCRIPTOR_INFO_TYPE_BUFFER>(descriptor_sets[0].clone())
            .dst_binding(0)
            .add_buffer_info(uniform_color_buffer_descriptor)
            .build(),
    );
    write_desc_sets.push(
        WriteDescriptorSet::builder::<DESCRIPTOR_INFO_TYPE_IMAGE>(descriptor_sets[0].clone())
            .dst_binding(1)
            .add_image_info(tex_descriptor)
            .build(),
    );
    device.update_descriptor_sets(&write_desc_sets, &[]);
    let mut vertex_spv_file = Cursor::new(&include_bytes!("vert.spv")[..]);
    let mut frag_spv_file = Cursor::new(&include_bytes!("frag.spv")[..]);

    let vertex_code =
        read_spv(&mut vertex_spv_file).expect("Failed to read vertex shader spv file");

    let frag_code = read_spv(&mut frag_spv_file).expect("Failed to read fragment shader spv file");

    let vertex_shader_module = ShaderModule::builder(device.clone(), &vertex_code)
        .build()
        .unwrap();

    let fragment_shader_module = ShaderModule::builder(device.clone(), &frag_code)
        .build()
        .unwrap();

    let pipeline_layout = PipelineLayout::builder(device.clone())
        .add_set_layout(desc_set_layout.clone())
        .build()
        .unwrap();

    let vertex_input_binding_descriptions = VertexInputBindingDescription::builder()
        .stride(std::mem::size_of::<Vertex>() as u32)
        .input_rate(VertexInputRate::VERTEX)
        .build();
    let vertex_input_state_info = PipelineVertexInputStateCreateInfo::builder()
        .add_vertex_input_attribute_description(VertexInputAttributeDescription {
            location: 0,
            binding: vertex_input_binding_descriptions,
            format: Format::R32G32B32A32_SFLOAT,
            offset: offset_of!(Vertex, pos) as u32,
        })
        .add_vertex_input_attribute_description(VertexInputAttributeDescription {
            location: 1,
            binding: vertex_input_binding_descriptions,
            format: Format::R32G32_SFLOAT,
            offset: offset_of!(Vertex, uv) as u32,
        })
        .build();
    let noop_stencil_state = StencilOpState {
        fail_op: StencilOp::KEEP,
        pass_op: StencilOp::KEEP,
        depth_fail_op: StencilOp::KEEP,
        compare_op: CompareOp::ALWAYS,
        ..Default::default()
    };

    let entry_name = unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"main\0") };
    // let op_feature = device.get_feature::<{ FeatureType::DeviceFeatures(PhysicalDeviceFeatures::LogicOp) }>().unwrap();
    let graphic_pipeline = Pipeline::builder(pipeline_layout.clone())
        .add_stage(
            PipelineShaderStageCreateInfo::builder(vertex_shader_module, entry_name)
                .stage(ShaderStageFlags::Vertex)
                .build(),
        )
        .add_stage(
            PipelineShaderStageCreateInfo::builder(fragment_shader_module, entry_name)
                .stage(ShaderStageFlags::Fragment)
                .build(),
        )
        .vertex_input_state(vertex_input_state_info)
        .viewport_state(
            PipelineViewportStateCreateInfo::builder()
                .viewport(Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: surface_resolution.width as f32,
                    height: surface_resolution.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                })
                .scissor(Rect2D {
                    extent: surface_resolution,
                    ..Default::default()
                })
                .build(),
        )
        .input_assembly_state(
            PipelineInputAssemblyStateCreateInfo::builder()
                .topology::<{ PrimitiveTopology::TriangleList }>()
                .build(),
        )
        .rasterization_state(
            PipelineRasterizationStateCreateInfo::builder()
                .front_face(FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0)
                .polygon_mode(PolygonMode::Fill)
                .build(),
        )
        .multisample_state(
            PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(SampleCountFlags::TYPE_1)
                .build(),
        )
        .depth_stencil_state(
            PipelineDepthStencilStateCreateInfo::builder()
                .depth_test_enable()
                .depth_write_enable()
                .depth_compare_op(CompareOp::LESS_OR_EQUAL)
                .front(noop_stencil_state.clone())
                .back(noop_stencil_state.clone())
                .depth_bounds(0.0, 1.0)
                .build(),
        )
        .color_blend_state(
            PipelineColorBlendStateCreateInfo::builder()
                // .logic_op(LogicOp::CLEAR, op_feature)
                .add_attachment(
                    PipelineColorBlendAttachmentState::builder()
                        .src_color_blend_factor(BlendFactor::SrcColor)
                        .dst_color_blend_factor(BlendFactor::OneMinusDstColor)
                        .color_blend_op(BlendOp::ADD)
                        .src_alpha_blend_factor(BlendFactor::Zero)
                        .dst_alpha_blend_factor(BlendFactor::Zero)
                        .alpha_blend_op(BlendOp::ADD)
                        .color_write_mask(ColorComponentFlags::RGBA)
                        .build(),
                )
                .build(),
        )
        .render_pass(renderpass.clone(), subpass_id0)
        .build()
        .unwrap();
    let present_complete_semaphore = Semaphore::new(device.clone()).unwrap();
    let rendering_complete_semaphore = Semaphore::new(device.clone()).unwrap();
    let mut submit_info_holder = Some(SubmitInfo::new());
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                present_queue.wait_idle().unwrap();
            }
            Event::MainEventsCleared => {
                let image = swapchain
                    .acquire_next_image_semaphore_only(
                        u64::MAX,
                        present_complete_semaphore.as_ref(),
                    )
                    .unwrap();
                let framebuffer = framebuffers.get(&image).unwrap();
                let render_pass_begin_info =
                    RenderPassBeginInfo::builder(renderpass.clone(), framebuffer.clone())
                        .render_area(surface_resolution.into())
                        .add_clear_value(ClearValue {
                            color: ClearColorValue {
                                float32: [0.0, 0.0, 0.0, 0.0],
                            },
                        })
                        .add_clear_value(ClearValue {
                            depth_stencil: ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            },
                        })
                        .build();
                let command_buffer = draw_command_buffer.take().unwrap();
                let command_buffer = command_buffer
                    .record(CommandBufferUsageFlags::ONE_TIME_SUBMIT, |command_buffer| {
                        command_buffer.cmd_begin_render_pass(
                            &render_pass_begin_info,
                            SubpassContents::INLINE,
                            |command_buffer| {
                                command_buffer.cmd_bind_descriptor_sets(
                                    PipelineBindPoint::GRAPHICS,
                                    &pipeline_layout,
                                    0,
                                    &descriptor_sets[..],
                                    &[],
                                );
                                command_buffer.cmd_bind_pipeline(
                                    PipelineBindPoint::GRAPHICS,
                                    &graphic_pipeline,
                                );
                                command_buffer.cmd_bind_vertex_buffers(
                                    0,
                                    &[vertex_input_buffer.clone()],
                                    &[0],
                                );
                                command_buffer.cmd_bind_index_buffer(
                                    index_buffer.clone(),
                                    0,
                                    IndexType::UINT32,
                                );
                                command_buffer.cmd_draw_indexed(
                                    index_buffer_data.len() as u32,
                                    1,
                                    0,
                                    0,
                                    1,
                                );
                            },
                        );
                    })
                    .unwrap();
                let mut submit_info = submit_info_holder.take().unwrap();
                submit_info.clear();
                submit_info.add_wait_semaphore(
                    present_complete_semaphore.clone(),
                    PipelineStageFlags::BottomOfPipe,
                );
                submit_info.add_command_buffer(command_buffer);
                submit_info.add_signal_semaphore(rendering_complete_semaphore.clone());
                let fence = draw_commands_reuse_fence.take().unwrap();
                let fence = present_queue
                    .submit(fence, vec![submit_info])
                    .expect("queue submit failed.");

                let mut present_info = PresentInfo::builder()
                    .add_swapchain_and_image(swapchain.clone(), &image)
                    .add_wait_semaphore(rendering_complete_semaphore.clone())
                    .build();
                present_queue.queue_present(&mut present_info).unwrap();

                let (fence, mut infos) = fence.wait().unwrap();
                let fence = fence.reset().unwrap();
                let mut submit_info = infos.pop().unwrap();
                let command_buffer = submit_info
                    .take_invalid_buffers()
                    .pop()
                    .unwrap()
                    .reset()
                    .unwrap();
                submit_info_holder = Some(submit_info);
                draw_command_buffer = Some(command_buffer);
                draw_commands_reuse_fence = Some(fence);
            }
            _ => (),
        }
    });
}
