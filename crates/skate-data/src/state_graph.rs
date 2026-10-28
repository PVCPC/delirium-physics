use crate::AssetError;
use std::{fs, path::Path};

pub mod attributes;
pub mod binding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAttribute {
    pub name: String,
    pub text: String,
    pub float_bits: u32,
    pub boolean_byte: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphElement {
    pub source_offset: usize,
    pub tag: String,
    pub attributes: Vec<GraphAttribute>,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGraph {
    pub elements: Vec<GraphElement>,
}

impl StateGraph {
    pub fn load(path: &Path) -> Result<Self, AssetError> {
        let bytes = fs::read(path)
            .map_err(|e| AssetError(format!("Cannot read graph {}: {e}", path.display())))?;
        Self::decode(&bytes)
            .map_err(|e| AssetError(format!("Invalid graph {}: {e}", path.display())))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AssetError> {
        let mut reader = Reader { bytes, offset: 0 };
        let (root, children) = reader.element()?;
        let mut elements = vec![root];
        let mut parents = vec![(0_usize, children)];
        while let Some((parent, remaining)) = parents.last_mut() {
            if *remaining == 0 {
                parents.pop();
                continue;
            }
            *remaining -= 1;
            let (element, children) = reader.element()?;
            let index = elements.len();
            elements[*parent].children.push(index);
            elements.push(element);
            parents.push((index, children));
        }
        if reader.offset != bytes.len() {
            return Err(reader.error("trailing bytes after the root element"));
        }
        Ok(Self { elements })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn error(&self, message: &str) -> AssetError {
        AssetError(format!("stategraph byte 0x{:X}: {message}", self.offset))
    }

    fn take(&mut self, count: usize) -> Result<&[u8], AssetError> {
        if count > self.bytes.len() - self.offset {
            return Err(self.error("truncated record"));
        }
        let begin = self.offset;
        self.offset += count;
        Ok(&self.bytes[begin..self.offset])
    }

    fn word(&mut self) -> Result<u32, AssetError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<String, AssetError> {
        let length = self.bytes[self.offset..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| self.error("unterminated string"))?;
        let offset = self.offset;
        let string = self.take(length + 1)?;
        std::str::from_utf8(&string[..length])
            .map(str::to_owned)
            .map_err(|_| AssetError(format!("stategraph byte 0x{offset:X}: non-UTF-8 string")))
    }

    fn element(&mut self) -> Result<(GraphElement, u32), AssetError> {
        let source_offset = self.offset;
        let tag = self.string()?;
        let attribute_count = self.word()?;
        if attribute_count as usize > (self.bytes.len() - self.offset) / 7 {
            return Err(self.error("attribute count exceeds remaining data"));
        }
        let mut attributes = Vec::with_capacity(attribute_count as usize);
        for _ in 0..attribute_count {
            attributes.push(GraphAttribute {
                name: self.string()?,
                text: self.string()?,
                float_bits: self.word()?,
                boolean_byte: self.take(1)?[0],
            });
        }
        let children = self.word()?;
        if children as usize > (self.bytes.len() - self.offset) / 9 {
            return Err(self.error("child count exceeds remaining data"));
        }
        Ok((
            GraphElement {
                source_offset,
                tag,
                attributes,
                children: Vec::new(),
            },
            children,
        ))
    }
}
