use egui::{
    self,
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
    TextureOptions, Ui, vec2, Vec2, Image, Sense,
};
use vorpal_core::ndarray;

#[derive(Default)]
pub struct ImageViewWidget {
    tex: Option<TextureId>,
}

impl ImageViewWidget {
    const OPTS: TextureOptions = TextureOptions::NEAREST;

    pub fn show(&mut self, ui: &mut Ui) -> egui::Response {
        if let Some(tex) = self.tex {
            let available = ui.available_size();
            if let Some(tex_meta) = ui.ctx().tex_manager().read().meta(tex) {
                let tex_size = Vec2::from(tex_meta.size.map(|v| v as f32));
                let tex_aspect = tex_size.x/tex_size.y;
                let size = if available.x/available.y < tex_aspect {
                    vec2(available.x, available.x / tex_aspect)
                } else {
                    vec2(available.y / tex_aspect, available.y)
                };

                return ui.add(Image::new((tex, size)).sense(Sense::click_and_drag()));
            }
        }

        ui.label("Texture not set, this is an error!")
    }

    pub fn set_image(&mut self, name: String, ctx: &egui::Context, image: ImageData) {
        if let Some(tex) = self.tex {
            ctx.tex_manager()
                .write()
                .set(tex, ImageDelta::full(image, Self::OPTS))
        } else {
            self.tex = Some(ctx.tex_manager().write().alloc(name, image, Self::OPTS))
        }
    }

    pub fn tex(&self) -> Option<TextureId> {
        self.tex
    }
}

/// Converts an image of 0 - 1 flaots into egui image data
pub fn array_to_imagedata(array: &ndarray::NdArray<f32>) -> ImageData {
    assert_eq!(
        array.shape().len(),
        3,
        "Array must have shape [width, height, 3]"
    );
    assert_eq!(array.shape()[2], 4, "Image must be RGBA");
    assert!(array.len() > 0);
    let dims = [array.shape()[0], array.shape()[1]];
    let mut rgba: Vec<u8> = array
        .data()
        .iter()
        .map(|value| (value.clamp(0., 1.) * 255.0) as u8)
        .collect();

    // Set alpha to one. TODO: UNDO THIS!!
    rgba.iter_mut()
        .skip(3)
        .step_by(4)
        .for_each(|v| *v = u8::MAX);

    ImageData::Color(ColorImage::from_rgba_unmultiplied(dims, &rgba).into())
}
