use crate::prelude::{PacketComponent, Size};
use crate::{throw_explain, PinnedLivelyResult};
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const COMPOUND_TAG_BIT: u8 = 10;

pub struct NbtAccounter {
    limit: u64,
    current: u64,
}

impl NbtAccounter {
    pub fn account_bytes(&mut self, bytes: u64) -> crate::prelude::Result<()> {
        if self.limit == 0 {
            return Ok(());
        }
        match self.current.checked_add(bytes) {
            Some(next) => {
                if next > self.limit {
                    throw_explain!(format!(
                        "Nbt tag too big, read {} bytes of allowed {}.",
                        next, self.limit
                    ));
                }
                self.current = next;
                Ok(())
            }
            None => throw_explain!("Overflowed bits in accounter."),
        }
    }
}

macro_rules! define_tags {
    ($(
        $tag:ident {
            const type = $backing_ty:ty;
            fn size($size_ref_ident:ident) {
                $($sizer_tt:tt)*
            },
            fn write($writer:ident, $write_ref_ident:ident) {
                $($writer_tt:tt)*
            },
            fn read($reader:ident, $accounter:ident, $depth:ident) {
                $($reader_tt:tt)*
            },
        }
    ),*) => {
        $(
            pub struct $tag;
        )*

        #[derive(Debug)]
        pub enum Tag {
            $(
                $tag($backing_ty),
            )*
        }

        impl Tag {
            pub fn get_tag_bit(&self) -> u8 {
                match self {
                    $(
                    Tag::$tag(_) => ${index(0)},
                    )*
                }
            }
        }

        pub fn load_tag<'a, R: $crate::prelude::AsyncRead + Unpin + ?Sized>(read: &'a mut R, bit: u8, depth: i32, accounter: &'a mut $crate::nbt::NbtAccounter) -> $crate::PinnedLivelyResult<'a, Tag> {
            Box::pin(async move {
                match bit {
                    $(
                    ${index(0)} => {
                        let $reader = read;
                        let $accounter = accounter;
                        let $depth = depth;
                        $($reader_tt)*
                    }
                    )*
                    _ => $crate::throw_explain!(format!("Invalid bit {} found while loading tag.", bit))
                }
            })
        }

        pub fn write_tag<'a, W: $crate::prelude::AsyncWrite + Unpin + ?Sized>(write: &'a mut W, tag: &'a Tag) -> $crate::PinnedLivelyResult<'a, ()> {
            Box::pin(async move {
                match tag {
                    $(
                    Tag::$tag($write_ref_ident) => {
                        let $writer = write;
                        $($writer_tt)*
                    }
                    )*
                }
            })
        }

        pub fn size_tag(tag: &Tag) -> $crate::prelude::Result<usize> {
            match tag {
                $(
                Tag::$tag($size_ref_ident) => {
                    $($sizer_tt)*
                }
                )*
            }
        }
    };
}

async fn read_string<R: AsyncRead + Unpin + ?Sized>(
    read: &mut R,
    accounter: &mut NbtAccounter,
) -> crate::prelude::Result<String> {
    let len = read.read_u16().await?;
    let mut bytes = vec![0u8; len as usize];
    read.read_exact(&mut bytes).await?;
    let string = cesu8::from_java_cesu8(&bytes)?.to_string();
    accounter.account_bytes(string.len() as u64)?;
    Ok(string)
}

async fn write_string<W: AsyncWrite + Unpin + ?Sized>(
    write: &mut W,
    reference: &String,
) -> crate::prelude::Result<()> {
    write.write_u16(reference.len() as u16).await?;
    write.write_all(&cesu8::to_java_cesu8(reference)).await?;
    Ok(())
}

fn size_string(reference: &String) -> crate::prelude::Result<usize> {
    Ok(2 + cesu8::to_java_cesu8(reference).len())
}

define_tags! {
    TagEnd {
        const type = ();
        fn size(_s) {
            Ok(0)
        },
        fn write(_w, _s) {
            Ok(())
        },
        fn read(_r, accounter, _d) {
            accounter.account_bytes(8)?;
            Ok(Tag::TagEnd(()))
        },
    },
    TagByte {
        const type = u8;
        fn size(_reference) {
            Ok(1)
        },
        fn write(writer, reference) {
            writer.write_u8(*reference).await?;
            Ok(())
        },

        fn read(reader, accounter, _d) {
            accounter.account_bytes(9)?;
            Ok(Tag::TagByte(reader.read_u8().await?))
        },
    },
    TagShort {
        const type = u16;
        fn size(_reference) {
            Ok(2)
        },
        fn write(writer, reference) {
            writer.write_u16(*reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(10)?;
            Ok(Tag::TagShort(reader.read_u16().await?))
        },
    },
    TagInt {
        const type = i32;
        fn size(_reference) {
            Ok(4)
        },
        fn write(writer, reference) {
            writer.write_i32(*reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(12)?;
            Ok(Tag::TagInt(reader.read_i32().await?))
        },
    },
    TagLong {
        const type = i64;
        fn size(_reference) {
            Ok(8)
        },
        fn write(writer, reference) {
            writer.write_i64(*reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(16)?;
            Ok(Tag::TagLong(reader.read_i64().await?))
        },
    },
    TagFloat {
        const type = f32;
        fn size(_reference) {
            Ok(4)
        },
        fn write(writer, reference) {
            writer.write_f32(*reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(12)?;
            Ok(Tag::TagFloat(reader.read_f32().await?))
        },
    },
    TagDouble {
        const type = f64;
        fn size(_reference) {
            Ok(8)
        },
        fn write(writer, reference) {
            writer.write_f64(*reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(16)?;
            Ok(Tag::TagDouble(reader.read_f64().await?))
        },
    },
    TagByteArray {
        const type = Vec<u8>;
        fn size(reference) {
            Ok(4 + reference.len())
        },
        fn write(writer, reference) {
            writer.write_i32(reference.len() as i32).await?;
            writer.write_all(reference).await?;
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(24)?;
            let len = reader.read_i32().await?;
            accounter.account_bytes(len as u64)?;
            let mut bytes = vec![0u8; len as usize];
            reader.read_exact(&mut bytes).await?;
            Ok(Tag::TagByteArray(bytes))
        },
    },
    TagString {
        const type = String;
        fn size(reference) {
            size_string(reference)
        },
        fn write(writer, reference) {
            write_string(writer, reference).await
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(36)?;
            Ok(Tag::TagString(read_string(reader, accounter).await?))
        },
    },
    TagList {
        const type = (u8, Vec<Tag>);
        fn size(reference) {
            Ok(5 + {
                let mut size = 0;
                for item in &reference.1 {
                    size += size_tag(item)?;
                }
                size
            })
        },
        fn write(writer, reference) {
            writer.write_u8(reference.0).await?;
            writer.write_i32(reference.1.len() as i32).await?;
            for tag in &reference.1 {
                write_tag(writer, tag).await?;
            }
            Ok(())
        },
        fn read(reader, accounter, depth) {
            accounter.account_bytes(37)?;
            if depth > 512 {
                throw_explain!("NBT tag too complex. Depth surpassed 512.")
            }
            let tag_byte = reader.read_u8().await?;
            let length = reader.read_i32().await?;
            accounter.account_bytes((4 * length) as u64)?;
            let mut v = Vec::with_capacity(length as usize);
            for _ in 0..length {
                v.push(load_tag(reader, tag_byte, depth + 1, accounter).await?);
            }
            Ok(Tag::TagList((tag_byte, v)))
        },
    },
    CompoundTag {
        const type = HashMap<String, Tag>;
        fn size(reference) {
            if reference.is_empty() {
                return Ok(1);
            }

            let mut size = 0;
            for (key, value) in reference {
                size += size_string(key)? + 1;
                size += size_tag(value)?;
            }
            Ok(size + 1)
        },
        fn write(writer, reference) {
            if reference.is_empty() {
                writer.write_u8(0).await?;
                return Ok(());
            }
            for (key, value) in reference {
                write_string(writer, key).await?;
                write_tag(writer, value).await?;
            }
            writer.write_u8(0).await?;
            Ok(())
        },
        fn read(reader, accounter, depth) {
            accounter.account_bytes(48)?;
            if depth > 512 {
                throw_explain!("NBT tag too complex. Depth surpassed 512.")
            }
            let mut map = HashMap::new();
            loop {
                let tag_byte = reader.read_u8().await?;
                if tag_byte == 0 {
                    break;
                }
                accounter.account_bytes(28)?;
                let key = read_string(reader, accounter).await?;
                let data = load_tag(reader, tag_byte, depth + 1, accounter).await?;

                if map.contains_key(&key) {
                    map.insert(key, data);
                    continue;
                }
                map.insert(key, data);
                accounter.account_bytes(36)?;
            }
            Ok(Tag::CompoundTag(map))
        },
    },
    TagIntArray {
        const type = Vec<i32>;
        fn size(reference) {
            Ok(4 + (4 * reference.len()))
        },
        fn write(writer, reference) {
            writer.write_i32(reference.len() as i32).await?;
            for item in reference {
                writer.write_i32(*item).await?;
            }
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(24)?;
            let len = reader.read_i32().await?;
            accounter.account_bytes((4 * len) as u64)?;
            let mut i_arr = vec![0i32; len as usize];
            for _ in 0..len {
                i_arr.push(reader.read_i32().await?);
            }
            Ok(Tag::TagIntArray(i_arr))
        },
    },
    TagLongArray {
        const type = Vec<i64>;
        fn size(reference) {
            Ok(4 + (8 * reference.len()))
        },
        fn write(writer, reference) {
            writer.write_i32(reference.len() as i32).await?;
            for item in reference {
                writer.write_i64(*item).await?;
            }
            Ok(())
        },
        fn read(reader, accounter, _d) {
            accounter.account_bytes(24)?;
            let len = reader.read_i32().await?;
            accounter.account_bytes((8 * len) as u64)?;
            let mut i_arr = vec![0i64; len as usize];
            for _ in 0..len {
                i_arr.push(reader.read_i64().await?);
            }
            Ok(Tag::TagLongArray(i_arr))
        },
    }
}

#[derive(Debug)]
pub enum EnsuredCompoundTag<const LIMIT: u64 = 0> {
    Tagged(Tag),
    NoTag,
}

impl<const LIMIT: u64, C> PacketComponent<C> for EnsuredCompoundTag<LIMIT> {
    type ComponentType = EnsuredCompoundTag<LIMIT>;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        _: &'a mut C,
        read: &'a mut A,
    ) -> PinnedLivelyResult<'a, Self::ComponentType> {
        Box::pin(async move {
            let b = read.read_u8().await?;
            if b == 0 {
                return Ok(EnsuredCompoundTag::NoTag);
            }
            if b != 10 {
                throw_explain!(format!(
                    "Invalid tag bit. Expected compound tag; received {}",
                    b
                ));
            }
            let mut accounter = NbtAccounter {
                limit: LIMIT,
                current: 0,
            };
            let _ = read_string(read, &mut accounter).await?;
            let tag = load_tag(read, b, 0, &mut accounter).await?;
            Ok(EnsuredCompoundTag::Tagged(tag))
        })
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        _: &'a mut C,
        write: &'a mut A,
    ) -> PinnedLivelyResult<'a, ()> {
        Box::pin(async move {
            match component_ref {
                EnsuredCompoundTag::Tagged(tag) => {
                    write.write_u8(10).await?;
                    write_string(write, &format!("")).await?;
                    write_tag(write, tag).await?;
                    Ok(())
                }
                EnsuredCompoundTag::NoTag => {
                    write.write_u8(0).await?;
                    Ok(())
                }
            }
        })
    }

    fn size(input: &Self::ComponentType, _: &mut C) -> crate::prelude::Result<Size> {
        match input {
            EnsuredCompoundTag::Tagged(tag) => {
                let dynamic_size = Size::Dynamic(3); // short 0 for str + byte tag
                Ok(dynamic_size + size_tag(tag)?)
            }
            EnsuredCompoundTag::NoTag => Ok(Size::Constant(1)),
        }
    }
}
