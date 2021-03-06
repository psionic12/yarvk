use crate::command::command_buffer::State::RECORDING;
use crate::command::command_buffer::{CommandBuffer, Level, RenderPassScope};
use crate::descriptor_pool::DescriptorSetLayout;
use crate::device::Device;
use crate::pipeline::color_blend_state::PipelineColorBlendStateCreateInfo;
use crate::pipeline::depth_stencil_state::PipelineDepthStencilStateCreateInfo;
use crate::pipeline::input_assembly_state::PipelineInputAssemblyStateCreateInfo;
use crate::pipeline::multisample_state::PipelineMultisampleStateCreateInfo;

use crate::pipeline::rasterization_state::PipelineRasterizationStateCreateInfo;
use crate::pipeline::shader_stage::{PipelineShaderStageCreateInfo};
use crate::pipeline::vertex_input_state::{
    PipelineVertexInputStateCreateInfo,
};
use crate::pipeline::viewport_state::PipelineViewportStateCreateInfo;
use crate::render_pass::subpass::SubpassIndex;
use crate::render_pass::RenderPass;

use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use crate::shader_module::ShaderModule;

pub mod color_blend_state;
pub mod depth_stencil_state;
pub mod input_assembly_state;
pub mod multisample_state;
pub mod pipeline_stage_flags;
pub mod primitive_topology;
pub mod rasterization_state;
pub mod shader_stage;
pub mod vertex_input_state;
pub mod viewport_state;

pub struct PipelineLayout {
    pub device: Arc<Device>,
    pub(crate) ash_vk_pipeline_layout: ash::vk::PipelineLayout,
}

impl PipelineLayout {
    pub fn builder(device: Arc<Device>) -> PipelineLayoutBuilder {
        PipelineLayoutBuilder {
            device,
            set_layouts: vec![],
            push_constant_ranges: vec![],
        }
    }
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        unsafe {
            // DONE VUID-vkDestroyPipelineLayout-pipelineLayout-02004
            // Host Synchronization: pipelineLayout
            self.device
                .ash_device
                .destroy_pipeline_layout(self.ash_vk_pipeline_layout, None);
        }
    }
}

pub struct PipelineLayoutBuilder {
    device: Arc<Device>,
    set_layouts: Vec<Arc<DescriptorSetLayout>>,
    push_constant_ranges: Vec<ash::vk::PushConstantRange>,
}

impl PipelineLayoutBuilder {
    pub fn add_set_layout(mut self, set_layout: Arc<DescriptorSetLayout>) -> Self {
        self.set_layouts.push(set_layout);
        self
    }
    pub fn add_push_constant_range(
        mut self,
        push_constant_range: ash::vk::PushConstantRange,
    ) -> Self {
        self.push_constant_ranges.push(push_constant_range);
        self
    }
    pub fn build(self) -> Result<Arc<PipelineLayout>, ash::vk::Result> {
        let vk_set_layouts = self
            .set_layouts
            .iter()
            .map(|layout| layout.ash_vk_descriptor_set_layout)
            .collect::<Vec<_>>();
        let create_info = ash::vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(vk_set_layouts.as_slice())
            .push_constant_ranges(self.push_constant_ranges.as_slice())
            .build();
        unsafe {
            // Host Synchronization: none
            let ash_vk_pipeline_layout = self
                .device
                .ash_device
                .create_pipeline_layout(&create_info, None)?;
            Ok(Arc::new(PipelineLayout {
                device: self.device,
                ash_vk_pipeline_layout,
            }))
        }
    }
}

#[derive(Default)]
pub struct PipelineTessellationStateCreateInfo {
    patch_control_points: u32,
}

impl PipelineTessellationStateCreateInfo {
    pub fn builder() -> PipelineTessellationStateCreateInfoBuilder {
        PipelineTessellationStateCreateInfoBuilder::default()
    }
    fn ash_builder(&self) -> ash::vk::PipelineTessellationStateCreateInfoBuilder {
        ash::vk::PipelineTessellationStateCreateInfo::builder()
            .patch_control_points(self.patch_control_points)
    }
}

#[derive(Default)]
pub struct PipelineTessellationStateCreateInfoBuilder {
    inner: PipelineTessellationStateCreateInfo,
}

impl PipelineTessellationStateCreateInfoBuilder {
    pub fn patch_control_points(mut self, patch_control_points: u32) -> Self {
        self.inner.patch_control_points = patch_control_points;
        self
    }
    pub fn build(self) -> PipelineTessellationStateCreateInfo {
        self.inner
    }
}

pub struct Pipeline {
    pub device: Arc<Device>,
    _render_pass_holder: Option<Arc<RenderPass>>,
    _shader_modules_holder: Vec<Arc<ShaderModule>>,
    ash_vk_pipeline: ash::vk::Pipeline,
}

impl Pipeline {
    pub fn builder<'a>(layout: Arc<PipelineLayout>) -> PipelineBuilder<'a> {
        PipelineBuilder {
            device: layout.device.clone(),
            flags: Default::default(),
            pipeline_vertex_input_state_create_info: Default::default(),
            stages: Default::default(),
            input_assembly_state: PipelineInputAssemblyStateCreateInfo::default(),
            viewport_state: PipelineViewportStateCreateInfo::default(),
            tessellation_state: Default::default(),
            rasterization_state: Default::default(),
            multisample_state: Default::default(),
            depth_stencil_state: Default::default(),
            color_blend_state: Default::default(),
            layout,
            dynamic_states: Default::default(),
            render_pass: None,
        }
    }
}

pub struct PipelineBuilder<'a> {
    device: Arc<Device>,
    flags: ash::vk::PipelineCreateFlags,
    pipeline_vertex_input_state_create_info: PipelineVertexInputStateCreateInfo,
    stages: FxHashMap<ash::vk::ShaderStageFlags, PipelineShaderStageCreateInfo<'a>>,
    input_assembly_state: PipelineInputAssemblyStateCreateInfo,
    viewport_state: PipelineViewportStateCreateInfo,
    tessellation_state: PipelineTessellationStateCreateInfo,
    rasterization_state: PipelineRasterizationStateCreateInfo,
    multisample_state: PipelineMultisampleStateCreateInfo,
    depth_stencil_state: PipelineDepthStencilStateCreateInfo,
    color_blend_state: PipelineColorBlendStateCreateInfo,
    layout: Arc<PipelineLayout>,
    dynamic_states: FxHashSet<ash::vk::DynamicState>,
    render_pass: Option<(Arc<RenderPass>, SubpassIndex)>,
}

impl<'a> PipelineBuilder<'a> {
    pub fn flags(mut self, flags: ash::vk::PipelineCreateFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn add_stage(mut self, stage: PipelineShaderStageCreateInfo<'a>) -> Self {
        // MUST VUID-VkGraphicsPipelineCreateInfo-stage-00726
        if let Some(_) = self.stages.insert(stage.stage, stage) {
            panic!("VUID-VkGraphicsPipelineCreateInfo-stage-00726")
        }
        self
    }
    pub fn vertex_input_state(
        mut self,
        vertex_input_state: PipelineVertexInputStateCreateInfo,
    ) -> Self {
        self.pipeline_vertex_input_state_create_info = vertex_input_state;
        self
    }
    pub fn input_assembly_state(
        mut self,
        input_assembly_state: PipelineInputAssemblyStateCreateInfo,
    ) -> Self {
        self.input_assembly_state = input_assembly_state;
        self
    }
    pub fn tessellation_state(
        mut self,
        tessellation_state: PipelineTessellationStateCreateInfo,
    ) -> Self {
        self.tessellation_state = tessellation_state;
        self
    }
    pub fn viewport_state(mut self, viewport_state: PipelineViewportStateCreateInfo) -> Self {
        self.viewport_state = viewport_state;
        self
    }
    pub fn rasterization_state(
        mut self,
        rasterization_state: PipelineRasterizationStateCreateInfo,
    ) -> Self {
        self.rasterization_state = rasterization_state;
        self
    }
    pub fn multisample_state(
        mut self,
        multisample_state: PipelineMultisampleStateCreateInfo,
    ) -> Self {
        self.multisample_state = multisample_state;
        self
    }
    pub fn depth_stencil_state(
        mut self,
        depth_stencil_state: PipelineDepthStencilStateCreateInfo,
    ) -> Self {
        self.depth_stencil_state = depth_stencil_state;
        self
    }
    pub fn color_blend_state(
        mut self,
        color_blend_state: PipelineColorBlendStateCreateInfo,
    ) -> Self {
        self.color_blend_state = color_blend_state;
        self
    }
    pub fn render_pass(mut self, render_pass: Arc<RenderPass>, subpass: SubpassIndex) -> Self {
        // DONE VUID-VkGraphicsPipelineCreateInfo-renderPass-06046
        self.render_pass = Some((render_pass, subpass));
        self
    }
    // All vendors suggest to avoid using pipeline derivatives, and the API design is a little
    // tricky (need build a tree to avoid reference loop. So I just leave it unimplemented
    // pub fn base_pipeline_handle(mut self, base_pipeline_handle: Arc<Pipeline>) -> Self {
    //     self.base_pipeline_handle = Some(base_pipeline_handle);
    //     self.flags |= ash::vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    //     self
    // }
    // pub fn base_pipeline_index(mut self, base_pipeline_index: i32) -> Self {
    //     self.base_pipeline_index = base_pipeline_index;
    //     self.flags |= ash::vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    //     self
    // }
    pub fn build(mut self) -> Result<Pipeline, ash::vk::Result> {
        // stages
        let mut shader_modules_holder = Vec::with_capacity(self.stages.len());
        let mut ash_vk_stages = Vec::with_capacity(self.stages.len());
        for (_, info) in self.stages {
            ash_vk_stages.push(info.ash_builder());
            shader_modules_holder.push(info.module);
        }
        // vertex input
        let ash_vk_vertex_input_state = self
            .pipeline_vertex_input_state_create_info
            .ash_builder()
            .build();
        // input assembly
        let ash_vk_input_assembly_state = self.input_assembly_state.ash_builder().build();
        // tessellation
        let ash_vk_tessellation_state = self.tessellation_state.ash_builder().build();
        // view port
        let ash_vk_viewport_state = self.viewport_state.ash_builder().build();
        // rasterization
        let ash_vk_rasterization_state = self.rasterization_state.ash_builder().build();
        // multisample
        let ash_vk_multisample_state = self.multisample_state.ash_builder().build();
        // depth stencil
        let ash_vk_depth_stencil_state = self.depth_stencil_state.ash_builder().build();
        // color blend
        let ash_vk_color_blend_state = self.color_blend_state.ash_builder().build();
        // dynamic states
        let ash_vk_dynamic_states = self.dynamic_states.into_iter().collect::<Vec<_>>();
        let ash_vk_pipeline_dynamic_state_create_info =
            ash::vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(ash_vk_dynamic_states.as_slice())
                .build();
        let mut create_info_builder = ash::vk::GraphicsPipelineCreateInfo::builder()
            .flags(self.flags)
            .stages(ash_vk_stages.as_slice())
            .vertex_input_state(&ash_vk_vertex_input_state)
            .input_assembly_state(&ash_vk_input_assembly_state)
            .tessellation_state(&ash_vk_tessellation_state)
            .viewport_state(&ash_vk_viewport_state)
            .rasterization_state(&ash_vk_rasterization_state)
            .multisample_state(&ash_vk_multisample_state)
            .depth_stencil_state(&ash_vk_depth_stencil_state)
            .color_blend_state(&ash_vk_color_blend_state)
            .layout(self.layout.ash_vk_pipeline_layout)
            .dynamic_state(&ash_vk_pipeline_dynamic_state_create_info);
        let mut render_pass_holder = None;
        if let Some((render_pass, subpass_index)) = self.render_pass {
            create_info_builder = create_info_builder
                .render_pass(render_pass.ash_vk_renderpass)
                .subpass(subpass_index.0);
            render_pass_holder = Some(render_pass);
        }
        let create_info = create_info_builder.build();
        // TODO pipeline caching
        let ash_vk_pipeline = unsafe {
            match self.device.ash_device.create_graphics_pipelines(
                ash::vk::PipelineCache::null(),
                &[create_info],
                None,
            ) {
                Ok(pipelines) => pipelines[0],
                Err((_, error)) => {
                    return Err(error.into());
                }
            }
        };
        Ok(Pipeline {
            device: self.device,
            _render_pass_holder: render_pass_holder,
            _shader_modules_holder: shader_modules_holder,
            ash_vk_pipeline,
        })
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            // TODO VUID-vkDestroyPipeline-pipeline-00765
            // Host Synchronization pipeline
            self.device
                .ash_device
                .destroy_pipeline(self.ash_vk_pipeline, None);
        }
    }
}

impl<const LEVEL: Level, const SCOPE: RenderPassScope> CommandBuffer<LEVEL, { RECORDING }, SCOPE> {
    // DONE VUID-vkCmdBindPipeline-commandBuffer-recording
    pub fn cmd_bind_pipeline(
        &mut self,
        pipeline_bind_point: ash::vk::PipelineBindPoint,
        pipeline: &Pipeline,
    ) {
        unsafe {
            // Host Synchronization: commandBuffer, VkCommandPool
            let _pool = self.command_pool.vk_command_pool.write();
            self.device.ash_device.cmd_bind_pipeline(
                self.vk_command_buffer,
                pipeline_bind_point,
                pipeline.ash_vk_pipeline,
            );
        }
    }
}
