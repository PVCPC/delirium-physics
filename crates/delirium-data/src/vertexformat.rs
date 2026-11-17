#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementType {
    Half,
    Half2,
    Half3,
    Half4,
    Float,
    Float2,
    Float3,
    Float4,
    UByte4,
    Colour,
    Dec3n,
    Unsized(u8),
}

impl ElementType {
    pub fn from_code(code: u8) -> ElementType {
        use ElementType::*;
        match code & 0xF {
            0 => Half,
            1 => Half2,
            2 => Half3,
            3 => Half4,
            4 => Float,
            5 => Float2,
            6 => Float3,
            7 => Float4,
            8 => UByte4,
            9 => Colour,
            10 => Dec3n,
            c => Unsized(c),
        }
    }

    pub fn code(self) -> u8 {
        use ElementType::*;
        match self {
            Half => 0,
            Half2 => 1,
            Half3 => 2,
            Half4 => 3,
            Float => 4,
            Float2 => 5,
            Float3 => 6,
            Float4 => 7,
            UByte4 => 8,
            Colour => 9,
            Dec3n => 10,
            Unsized(c) => c,
        }
    }

    pub fn size(self) -> u8 {
        use ElementType::*;
        match self {
            Half => 2,
            Half2 | Float | UByte4 | Colour | Dec3n => 4,
            Half3 => 6,
            Half4 | Float2 => 8,
            Float3 => 12,
            Float4 => 16,
            Unsized(_) => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Position,
    BlendWeight,
    BlendIndex,
    Normal,
    Colour,
    Specular,
    TexCoord(u8),
    Tangent(u8),
}

impl Channel {
    pub fn from_number(n: u8) -> Channel {
        match n & 0xF {
            0 => Channel::Position,
            1 => Channel::BlendWeight,
            2 => Channel::BlendIndex,
            3 => Channel::Normal,
            4 => Channel::Colour,
            5 => Channel::Specular,
            n @ 6..=13 => Channel::TexCoord(n - 6),
            n => Channel::Tangent(n - 14),
        }
    }

    pub fn number(self) -> u8 {
        match self {
            Channel::Position => 0,
            Channel::BlendWeight => 1,
            Channel::BlendIndex => 2,
            Channel::Normal => 3,
            Channel::Colour => 4,
            Channel::Specular => 5,
            Channel::TexCoord(i) => 6 + i,
            Channel::Tangent(i) => 14 + i,
        }
    }

    fn bit(self) -> u16 {
        1 << self.number()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChannelSet(pub u16);

impl ChannelSet {
    pub fn has(self, c: Channel) -> bool {
        self.0 & c.bit() != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexFormat {
    pub channels: ChannelSet,
    pub vertex_size: u8,
    pub packed_positions: bool,
    pub packed: bool,
    pub types: [ElementType; 16],
}

impl Default for VertexFormat {
    fn default() -> Self {
        VertexFormat {
            channels: ChannelSet(0),
            vertex_size: 0,
            packed_positions: false,
            packed: false,
            types: [ElementType::Half; 16],
        }
    }
}

pub const PACKED_LAYOUT: [(Channel, ElementType); 16] = {
    use Channel as C;
    use ElementType::{Colour as Argb, Float4};
    [
        (C::Position, Float4), (C::Normal, Float4), (C::Tangent(0), Float4), (C::TexCoord(0), Float4),
        (C::TexCoord(1), Float4), (C::TexCoord(2), Float4), (C::TexCoord(3), Float4), (C::TexCoord(4), Float4),
        (C::TexCoord(5), Float4), (C::TexCoord(6), Float4), (C::TexCoord(7), Float4), (C::BlendWeight, Float4),
        (C::Tangent(1), Float4), (C::BlendIndex, Argb), (C::Colour, Argb), (C::Specular, Argb),
    ]
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeAsk {
    Default,
    Type(ElementType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelRule {
    pub channel: Channel,
    pub default: ElementType,
    pub packed_forces_float4: bool,
}

impl ChannelRule {
    fn pick(self, packed: bool, ask: TypeAsk) -> ElementType {
        if self.packed_forces_float4 && packed {
            ElementType::Float4
        } else {
            match ask {
                TypeAsk::Default => self.default,
                TypeAsk::Type(t) => t,
            }
        }
    }
}

pub mod rules {
    use super::{Channel, ChannelRule, ElementType};

    pub const BLEND_WEIGHT: ChannelRule =
        ChannelRule { channel: Channel::BlendWeight, default: ElementType::Float2, packed_forces_float4: true };
    pub const BLEND_INDEX: ChannelRule =
        ChannelRule { channel: Channel::BlendIndex, default: ElementType::Colour, packed_forces_float4: false };
    pub const NORMAL: ChannelRule =
        ChannelRule { channel: Channel::Normal, default: ElementType::Float3, packed_forces_float4: true };
    pub const COLOUR: ChannelRule =
        ChannelRule { channel: Channel::Colour, default: ElementType::Colour, packed_forces_float4: false };
    pub const SPECULAR: ChannelRule =
        ChannelRule { channel: Channel::Specular, default: ElementType::Colour, packed_forces_float4: false };

    pub fn tex_coord(set: u8) -> ChannelRule {
        ChannelRule { channel: Channel::TexCoord(set), default: ElementType::Float2, packed_forces_float4: true }
    }

    pub fn tangent(i: u8) -> ChannelRule {
        ChannelRule { channel: Channel::Tangent(i), default: ElementType::Float3, packed_forces_float4: true }
    }
}

impl VertexFormat {
    fn switch(&mut self, channel: Channel, on: bool, ty: ElementType) {
        self.types[channel.number() as usize] = ty;
        let had = self.channels.has(channel);
        if on && !had {
            self.channels.0 |= channel.bit();
        } else if had {
            self.channels.0 &= !channel.bit();
        }
    }

    pub fn channel_count(&self) -> u32 {
        self.channels.0.count_ones()
    }

    pub fn set_channel(&mut self, rule: ChannelRule, on: bool, ask: TypeAsk) {
        let ty = rule.pick(self.packed, ask);
        self.switch(rule.channel, on, ty);
        self.update_vertex_size();
    }

    pub fn set_position(&mut self, on: bool, ask: TypeAsk, packed_positions: bool) {
        let ty = if self.packed {
            ElementType::Float4
        } else {
            match ask {
                TypeAsk::Default if packed_positions => ElementType::Float4,
                TypeAsk::Default => ElementType::Float3,
                TypeAsk::Type(t) => t,
            }
        };
        self.switch(Channel::Position, on, ty);
        self.update_vertex_size();
        self.packed_positions = packed_positions;
    }

    pub fn update_vertex_size(&mut self) {
        let mut size: u32 = 0;
        for n in 0..16u8 {
            let c = Channel::from_number(n);
            if self.channels.has(c) {
                size += self.types[n as usize].size() as u32;
            }
        }
        if self.packed {
            size = (size + 15) & !15;
        }
        self.vertex_size = size as u8;
    }

    pub fn element_offset(&self, channel: Channel) -> u32 {
        let mut offset: u32 = 0;
        if self.packed {
            for (c, ty) in PACKED_LAYOUT {
                if self.channels.has(c) {
                    if c == channel {
                        return offset;
                    }
                    offset += ty.size() as u32;
                }
            }
        } else {
            for n in 0..16u8 {
                let c = Channel::from_number(n);
                if c == channel {
                    return offset;
                }
                if self.channels.has(c) {
                    offset += self.types[n as usize].size() as u32;
                }
            }
        }
        offset
    }

    pub fn describe(&self) -> String {
        let channels: Vec<String> = (0..16u8)
            .map(Channel::from_number)
            .filter(|&c| self.channels.has(c))
            .map(|c| format!("{c:?}:{:?}", self.types[c.number() as usize]))
            .collect();
        format!("size {:2}{} {}", self.vertex_size, if self.packed { " packed" } else { "" }, channels.join(" "))
    }

    pub fn packed_from(src: &VertexFormat) -> VertexFormat {
        let mut f = VertexFormat { packed: true, ..VertexFormat::default() };
        for (c, ty) in PACKED_LAYOUT {
            if src.channels.has(c) {
                f.switch(c, true, ty);
            }
        }
        f.update_vertex_size();
        f
    }
}
