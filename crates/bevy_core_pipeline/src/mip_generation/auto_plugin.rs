use bevy_app::{App, Plugin, PostUpdate};
use bevy_asset::{AssetEvent, AssetId, Assets};
use bevy_diagnostic::DiagnosticPath;
use bevy_ecs::{
    message::{Message, MessageReader, MessageWriter},
    schedule::IntoScheduleConfigs,
    system::{Res, ResMut},
};
use bevy_image::Image;
use bevy_render::{
    render_asset::RenderAssets,
    render_resource::{PipelineCache, TextureDimension, TextureUsages},
    renderer::RenderContext,
    texture::GpuImage,
    Extract, ExtractSchedule, Render, RenderApp,
    RenderSystems::{self},
};

use crate::mip_generation::{
    generate_mips_for_phase, MipGenerationJobs, MipGenerationPhaseId, MipGenerationPipelines,
    TEXTURE_FORMATS,
};

/// Automatically generates mipmaps for all images.
pub struct AutoMipGenerationPlugin;

const MIP_GENERATION_PHASE_ID: MipGenerationPhaseId = MipGenerationPhaseId(0);

impl Plugin for AutoMipGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MipGenerationRequest>()
            .add_systems(PostUpdate, submit_requests_to_mipmap);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(Render, generate_mips.in_set(RenderSystems::PrepareAssets))
            .add_systems(ExtractSchedule, extract_requests_to_mipmap);
    }
}

/// Number of mipmaps that have been generated.
pub const MIPMAPS_GENERATED: DiagnosticPath = DiagnosticPath::const_new("mipmaps_generated");

/// Number of mipmaps that are currently queued for automatic mipmap generation.
pub const MIPMAPS_QUEUED: DiagnosticPath = DiagnosticPath::const_new("mipmaps_queued");

fn generate_mips(
    mip_generation_jobs: Res<MipGenerationJobs>,
    pipeline_cache: Res<PipelineCache>,
    mip_generation_pipelines: Option<Res<MipGenerationPipelines>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    mut ctx: RenderContext,
) {
    let Some(mip_generation_pipelines) = mip_generation_pipelines else {
        return;
    };
    generate_mips_for_phase(
        MIP_GENERATION_PHASE_ID,
        &mip_generation_jobs,
        &pipeline_cache,
        &mip_generation_pipelines,
        &gpu_images,
        &mut ctx,
    );
}

#[derive(Message)]
pub struct MipGenerationRequest {
    asset_id: AssetId<Image>,
}

fn extract_requests_to_mipmap(
    mut events: Extract<MessageReader<MipGenerationRequest>>,
    mut mip_generation_jobs: ResMut<MipGenerationJobs>,
) {
    for request in events.read() {
        mip_generation_jobs.add(MIP_GENERATION_PHASE_ID, request.asset_id);
    }
}

fn submit_requests_to_mipmap(
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut request_events: MessageWriter<MipGenerationRequest>,
    mut images: ResMut<Assets<Image>>,
) {
    for image_id in image_events.read().filter_map(|event| match event {
        AssetEvent::Modified { id } | AssetEvent::Added { id } => Some(id),
        _ => None,
    }) {
        if let Some(mut image) = images.get_mut(*image_id) {
            if image.texture_descriptor.mip_level_count == 1
                && image.texture_descriptor.dimension == TextureDimension::D2
                && TEXTURE_FORMATS
                    .iter()
                    .any(|(format, _)| format == &image.texture_descriptor.format)
            {
                image.texture_descriptor.mip_level_count = image_mip_level_count(&image);
                image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;

                request_events.write(MipGenerationRequest {
                    asset_id: *image_id,
                });
            }
        }
    }
}

/// Returns the number of mipmap levels that the image should possess.
///
/// This will be equal to the maximum number of mipmap levels that an image
/// of the appropriate size can have.
fn image_mip_level_count(image: &Image) -> u32 {
    32 - image.width().max(image.height()).leading_zeros()
}
