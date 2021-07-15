use bevy::{
    math::*,
    render2::{
        color::Color,
    },
};

#[derive(Debug, Clone)]
pub struct Bloom {
    pub threshold: f32,
    pub intensity: f32,
    pub scatter: f32,
    pub tint: Color,
    pub clamp: f32,
}

impl Default for Bloom {
    fn default() -> Self {
        Self {
            threshold: 0.9,
            intensity: 0.0,
            scatter: 0.7,
            tint: Color::WHITE,
            clamp: 65472.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChannelMixing {
    red: Color,
    blue: Color,
    green: Color,
}

impl ChannelMixing {
    pub fn red(&self) -> Color {
        self.red.clone()
    }

    pub fn blue(&self) -> Color {
        self.blue.clone()
    }

    pub fn green(&self) -> Color {
        self.green.clone()
    }

    pub fn red_mut(&mut self) -> &mut Color {
        &mut self.red
    }

    pub fn blue_mut(&mut self) -> &mut Color {
        &mut self.blue
    }

    pub fn green_mut(&mut self) -> &mut Color {
        &mut self.green
    }
}

impl Default for ChannelMixing {
    fn default() -> Self {
        Self {
            red: Color::RED,
            blue: Color::BLUE,
            green: Color::GREEN,
        }
    }
}

impl From<ChannelMixing> for Mat3 {
    fn from(value: ChannelMixing) -> Self {
        Mat3::from_cols(
            Vec4::from(value.red).xyz(),
            Vec4::from(value.blue).xyz(),
            Vec4::from(value.green).xyz(),
        ).transpose()
    }
}

#[derive(Debug, Clone)]
pub struct NormalTonemapping;

#[derive(Debug, Clone)]
pub struct ACESTonemapping;