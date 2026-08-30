use std::collections::BTreeMap;
use std::path::PathBuf;

use fontdb::{Database, Family, Query};
use fontdue::layout::{
    CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign,
};
use fontdue::{Font, FontSettings};
use image::imageops::FilterType;

use crate::frame;

pub const LOGICAL_WIDTH: usize = 960;
pub const LOGICAL_HEIGHT: usize = 376;
const CHANNELS: usize = 3;

pub fn system_font_families() -> Vec<String> {
    let mut database = Database::new();
    database.load_system_fonts();

    let mut families = BTreeMap::new();
    for face in database.faces() {
        if let Some((family, _)) = face.families.first() {
            families
                .entry(family.to_lowercase())
                .or_insert_with(|| family.clone());
        }
    }
    families.into_values().collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClockStyle {
    pub background_path: Option<PathBuf>,
    pub background_color: [u8; 3],
    pub font_family: Option<String>,
    pub font_size: f32,
    pub text_color: [u8; 3],
    pub position: Option<(i32, i32)>,
}

impl Default for ClockStyle {
    fn default() -> Self {
        Self {
            background_path: None,
            background_color: [12, 16, 24],
            font_family: None,
            font_size: 120.0,
            text_color: [255, 255, 255],
            position: None,
        }
    }
}

pub struct ClockRenderer {
    style: ClockStyle,
    background: Vec<u8>,
    font: Font,
}

impl ClockRenderer {
    pub fn new(style: ClockStyle) -> Result<Self, String> {
        if !style.font_size.is_finite() || style.font_size <= 0.0 {
            return Err("font size must be a positive number".to_owned());
        }

        let background = load_background(&style)?;
        let font = load_font(style.font_family.as_deref())?;
        Ok(Self {
            style,
            background,
            font,
        })
    }

    pub fn render_logical(&self, text: &str) -> Vec<u8> {
        let mut logical_rgb = self.background.clone();
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        let settings = match self.style.position {
            None => LayoutSettings {
                max_width: Some(LOGICAL_WIDTH as f32),
                max_height: Some(LOGICAL_HEIGHT as f32),
                horizontal_align: HorizontalAlign::Center,
                vertical_align: VerticalAlign::Middle,
                ..LayoutSettings::default()
            },
            Some((x, y)) => LayoutSettings {
                x: x as f32,
                y: y as f32,
                ..LayoutSettings::default()
            },
        };
        layout.reset(&settings);
        layout.append(
            &[&self.font],
            &TextStyle::new(text, self.style.font_size, 0),
        );

        for glyph in layout.glyphs() {
            let (_, coverage) = self.font.rasterize_config(glyph.key);
            for glyph_y in 0..glyph.height {
                for glyph_x in 0..glyph.width {
                    let x = glyph.x.floor() as i32 + glyph_x as i32;
                    let y = glyph.y.floor() as i32 + glyph_y as i32;
                    if x < 0 || y < 0 || x >= LOGICAL_WIDTH as i32 || y >= LOGICAL_HEIGHT as i32 {
                        continue;
                    }

                    let alpha = coverage[glyph_y * glyph.width + glyph_x] as u16;
                    let pixel = (y as usize * LOGICAL_WIDTH + x as usize) * CHANNELS;
                    for channel in 0..CHANNELS {
                        let foreground = self.style.text_color[channel] as u16;
                        let background = logical_rgb[pixel + channel] as u16;
                        logical_rgb[pixel + channel] =
                            ((foreground * alpha + background * (255 - alpha) + 127) / 255) as u8;
                    }
                }
            }
        }

        // The DS339 reserves 0xFF as a frame marker. Applying the same color
        // adjustment here keeps the preview pixel-identical to the sent image.
        logical_rgb.iter_mut().for_each(|channel| {
            if *channel == 0xFF {
                *channel = 0xFE;
            }
        });
        logical_rgb
    }

    pub fn native_bgr(logical_rgb: &[u8]) -> Result<Vec<u8>, String> {
        let expected = LOGICAL_WIDTH * LOGICAL_HEIGHT * CHANNELS;
        if logical_rgb.len() != expected {
            return Err(format!(
                "invalid logical pixel buffer length: {} (expected {expected})",
                logical_rgb.len()
            ));
        }
        Ok(rotate_clockwise_to_native_bgr(logical_rgb))
    }
}

fn load_background(style: &ClockStyle) -> Result<Vec<u8>, String> {
    match &style.background_path {
        Some(path) => image::open(path)
            .map_err(|error| format!("failed to load background {}: {error}", path.display()))
            .map(|image| {
                image
                    .resize_to_fill(
                        LOGICAL_WIDTH as u32,
                        LOGICAL_HEIGHT as u32,
                        FilterType::Lanczos3,
                    )
                    .to_rgb8()
                    .into_raw()
            }),
        None => Ok(style
            .background_color
            .into_iter()
            .cycle()
            .take(LOGICAL_WIDTH * LOGICAL_HEIGHT * CHANNELS)
            .collect()),
    }
}

fn load_font(requested_family: Option<&str>) -> Result<Font, String> {
    let mut database = Database::new();
    database.load_system_fonts();

    let id = match requested_family {
        Some(family) => database
            .query(&Query {
                families: &[Family::Name(family)],
                ..Query::default()
            })
            .ok_or_else(|| format!("font family was not found: {family}"))?,
        None => database
            .query(&Query {
                families: &[Family::SansSerif],
                ..Query::default()
            })
            .ok_or_else(|| "no usable system font was found".to_owned())?,
    };

    database
        .with_face_data(id, |data, collection_index| {
            Font::from_bytes(
                data,
                FontSettings {
                    collection_index,
                    ..FontSettings::default()
                },
            )
        })
        .ok_or_else(|| "failed to read the selected system font".to_owned())?
        .map_err(str::to_owned)
}

fn rotate_clockwise_to_native_bgr(logical_rgb: &[u8]) -> Vec<u8> {
    debug_assert_eq!(logical_rgb.len(), LOGICAL_WIDTH * LOGICAL_HEIGHT * CHANNELS);
    debug_assert_eq!(frame::WIDTH, LOGICAL_HEIGHT);
    debug_assert_eq!(frame::HEIGHT, LOGICAL_WIDTH);

    let mut native_bgr = vec![0_u8; frame::WIDTH * frame::HEIGHT * CHANNELS];
    for y in 0..LOGICAL_HEIGHT {
        for x in 0..LOGICAL_WIDTH {
            let source = (y * LOGICAL_WIDTH + x) * CHANNELS;
            let native_x = LOGICAL_HEIGHT - 1 - y;
            let native_y = x;
            let destination = (native_y * frame::WIDTH + native_x) * CHANNELS;
            native_bgr[destination..destination + CHANNELS].copy_from_slice(&[
                logical_rgb[source + 2],
                logical_rgb[source + 1],
                logical_rgb[source],
            ]);
        }
    }
    native_bgr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_rgb_canvas_to_native_bgr() {
        let mut source = vec![0_u8; LOGICAL_WIDTH * LOGICAL_HEIGHT * CHANNELS];
        source[..3].copy_from_slice(&[10, 20, 30]);

        let native = rotate_clockwise_to_native_bgr(&source);
        let destination = (LOGICAL_HEIGHT - 1) * CHANNELS;

        assert_eq!(&native[destination..destination + 3], &[30, 20, 10]);
    }

    #[test]
    fn rejects_invalid_font_size_before_loading_fonts() {
        let style = ClockStyle {
            font_size: 0.0,
            ..ClockStyle::default()
        };

        assert!(ClockRenderer::new(style).is_err());
    }

    #[test]
    fn top_left_placement_is_available_for_configuration() {
        let style = ClockStyle {
            position: Some((10, 20)),
            ..ClockStyle::default()
        };
        assert_eq!(style.position, Some((10, 20)));
    }

    #[test]
    fn logical_render_uses_transmittable_color_values() {
        let renderer = ClockRenderer::new(ClockStyle::default()).unwrap();
        let logical = renderer.render_logical("12:34:56");

        assert_eq!(logical.len(), LOGICAL_WIDTH * LOGICAL_HEIGHT * CHANNELS);
        assert!(!logical.contains(&0xFF));
    }

    #[test]
    fn system_font_family_names_are_sorted_and_unique() {
        let families = system_font_families();
        assert!(!families.is_empty());
        assert!(
            families
                .windows(2)
                .all(|pair| pair[0].to_lowercase() < pair[1].to_lowercase())
        );
    }
}
