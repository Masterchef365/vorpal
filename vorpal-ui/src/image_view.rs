use eframe::{
    egui::{self, TextureOptions, Ui},
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
};
use vorpal_core::ndarray;


#[derive(Default)]
pub struct ImageViewWidget {
    tex: Option<TextureId>,
}

impl ImageViewWidget {
    const OPTS: TextureOptions = TextureOptions::NEAREST;

    pub fn show(&mut self, ui: &mut Ui) {
        if let Some(tex) = self.tex {
            ui.image(tex, ui.available_size());
        }
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

    ImageData::Color(ColorImage::from_rgba_unmultiplied(dims, &rgba))
}

