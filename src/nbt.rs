use crate::throw_explain;
use crate::transport::packet::PacketComponent;
use std::collections::hash_map::Keys;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const COMPOUND_TAG_BIT: u8 = 10;

pub struct NbtAccounter {
    limit: u64,
    current: u64,
}

impl NbtAccounter {
    fn account_bits(&mut self, bits: u64) -> crate::prelude::Result<()> {
        match self.current.checked_add(bits) {
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

#[derive(Debug, Clone)]
pub enum Tag {
    EndTag,
    ByteTag(u8),
    ShortTag(i16),
    IntTag(i32),
    LongTag(i64),
    FloatTag(f32),
    DoubleTag(f64),
    ByteArrayTag(Vec<u8>),
    StringTag(String),
    ListTag(u8, Vec<Tag>),
    CompoundTag(CompoundTag),
    IntArrayTag(Vec<i32>),
    LongArrayTag(Vec<i64>),
}

impl Tag {
    pub fn byte_tag<I: Into<u8>>(i: I) -> Tag {
        Tag::ByteTag(i.into())
    }
    pub fn short_tag<I: Into<i16>>(i: I) -> Tag {
        Tag::ShortTag(i.into())
    }
    pub fn int_tag<I: Into<i32>>(i: I) -> Tag {
        Tag::IntTag(i.into())
    }
    pub fn long_tag<I: Into<i64>>(i: I) -> Tag {
        Tag::LongTag(i.into())
    }
    pub fn float_tag<I: Into<f32>>(i: I) -> Tag {
        Tag::FloatTag(i.into())
    }
    pub fn double_tag<I: Into<f64>>(i: I) -> Tag {
        Tag::DoubleTag(i.into())
    }
    pub fn byte_array_tag<I: Into<Vec<u8>>>(i: I) -> Tag {
        Tag::ByteArrayTag(i.into())
    }
    pub fn string_tag<I: Into<String>>(i: I) -> Tag {
        Tag::StringTag(i.into())
    }
    pub fn int_array_tag<I: Into<Vec<i32>>>(i: I) -> Tag {
        Tag::IntArrayTag(i.into())
    }
    pub fn long_array_tag<I: Into<Vec<i64>>>(i: I) -> Tag {
        Tag::LongArrayTag(i.into())
    }
}

impl From<u8> for Tag {
    fn from(into: u8) -> Self {
        Tag::byte_tag(into)
    }
}

impl From<i16> for Tag {
    fn from(into: i16) -> Self {
        Tag::short_tag(into)
    }
}

impl From<i32> for Tag {
    fn from(into: i32) -> Self {
        Tag::int_tag(into)
    }
}

impl From<i64> for Tag {
    fn from(into: i64) -> Self {
        Tag::long_tag(into)
    }
}

impl From<f32> for Tag {
    fn from(into: f32) -> Self {
        Tag::float_tag(into)
    }
}

impl From<f64> for Tag {
    fn from(into: f64) -> Self {
        Tag::double_tag(into)
    }
}

impl From<Vec<u8>> for Tag {
    fn from(into: Vec<u8>) -> Self {
        Tag::byte_array_tag(into)
    }
}

impl From<String> for Tag {
    fn from(into: String) -> Self {
        Tag::string_tag(into)
    }
}

impl From<&str> for Tag {
    fn from(into: &str) -> Self {
        Tag::string_tag(into)
    }
}

impl From<&String> for Tag {
    fn from(into: &String) -> Self {
        Tag::string_tag(into)
    }
}

impl From<Vec<i32>> for Tag {
    fn from(into: Vec<i32>) -> Self {
        Tag::int_array_tag(into)
    }
}

impl From<Vec<i64>> for Tag {
    fn from(into: Vec<i64>) -> Self {
        Tag::long_array_tag(into)
    }
}

impl From<CompoundTag> for Tag {
    fn from(ctg: CompoundTag) -> Self {
        Tag::CompoundTag(ctg)
    }
}

impl From<Vec<CompoundTag>> for Tag {
    fn from(into: Vec<CompoundTag>) -> Self {
        Tag::ListTag {
            0: COMPOUND_TAG_BIT,
            1: into.into_iter().map(Tag::CompoundTag).collect(),
        }
    }
}

impl From<Vec<Tag>> for Tag {
    fn from(into: Vec<Tag>) -> Self {
        Tag::ListTag {
            0: COMPOUND_TAG_BIT,
            1: into,
        }
    }
}

impl Tag {
    pub fn get_bit(&self) -> u8 {
        match self {
            Tag::EndTag => 0,
            Tag::ByteTag(_) => 1,
            Tag::ShortTag(_) => 2,
            Tag::IntTag(_) => 3,
            Tag::LongTag(_) => 4,
            Tag::FloatTag(_) => 5,
            Tag::DoubleTag(_) => 6,
            Tag::ByteArrayTag(_) => 7,
            Tag::StringTag(_) => 8,
            Tag::ListTag(_, _) => 9,
            Tag::CompoundTag(_) => COMPOUND_TAG_BIT,
            Tag::IntArrayTag(_) => 11,
            Tag::LongArrayTag(_) => 12,
        }
    }
}

async fn skip_bytes<R: AsyncRead + Unpin + ?Sized, I: Into<u64>>(
    read: &mut R,
    i: I,
) -> crate::prelude::Result<()> {
    let taken = i.into();
    let a = tokio::io::copy(&mut read.take(taken), &mut tokio::io::sink()).await?;
    if taken != a {
        throw_explain!(format!(
            "Failed to skip correct number of bytes, only skipped {} out of {}",
            a, taken
        ));
    }
    Ok(())
}

async fn skip_string<R: AsyncRead + Unpin + ?Sized>(read: &mut R) -> crate::prelude::Result<()> {
    let skipped = u16::decode(read).await?;
    skip_bytes(read, skipped).await?;
    Ok(())
}

async fn read_string<R: AsyncRead + Unpin + ?Sized>(
    read: &mut R,
) -> crate::prelude::Result<String> {
    let str_len = u16::decode(read).await?;
    if str_len == 0 {
        return Ok(String::new());
    }
    let mut bytes = vec![0u8; str_len as usize];
    let mut bytes_read = 0;
    while bytes_read < bytes.len() {
        match read.read(&mut bytes[bytes_read..]).await? {
            0 => throw_explain!("Invalid NBT string, under read."),
            n => bytes_read += n,
        }
    }
    Ok(cesu8::from_java_cesu8(&bytes)?.to_string())
}

fn size_string(string: &str) -> usize {
    4 + cesu8::to_java_cesu8(string).len()
}

async fn write_string<W: AsyncWrite + Unpin + ?Sized>(
    write: &mut W,
    string: &String,
) -> crate::prelude::Result<()> {
    write.write_u16(string.len() as u16).await?;
    write.write_all(&cesu8::to_java_cesu8(string)).await?;
    Ok(())
}

async fn write_compound_tag<W: AsyncWrite + Unpin + ?Sized>(
    tag: &CompoundTag,
    write: &mut W,
) -> crate::prelude::Result<()> {
    for (key, value) in &tag.mappings {
        let id = value.get_bit();
        write.write_u8(id).await?;
        if id == 0 {
            return Ok(());
        }
        write_string(write, key).await?;
        write_tag(value, write).await?;
    }
    write.write_u8(0).await?;
    Ok(())
}

fn size_compound_tag(tag: &CompoundTag) -> usize {
    let mut size = 0;
    for (key, value) in &tag.mappings {
        let id = value.get_bit();
        if id == 0 {
            return size + 1;
        }
        size += 1 + size_string(key);
        size += size_tag(value);
    }
    size + 1
}

fn write_tag<'a, W: AsyncWrite + Unpin + ?Sized>(
    tag: &'a Tag,
    write: &'a mut W,
) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
    Box::pin(async move {
        match tag {
            Tag::EndTag => Ok(()),
            Tag::ByteTag(byte) => write.write_u8(*byte).await.map_err(Into::into),
            Tag::ShortTag(short) => write.write_i16(*short).await.map_err(Into::into),
            Tag::IntTag(int) => write.write_i32(*int).await.map_err(Into::into),
            Tag::LongTag(long) => write.write_i64(*long).await.map_err(Into::into),
            Tag::FloatTag(float) => write.write_f32(*float).await.map_err(Into::into),
            Tag::DoubleTag(double) => write.write_f64(*double).await.map_err(Into::into),
            Tag::ByteArrayTag(b_arr) => {
                write.write_i32(b_arr.len() as i32).await?;
                write.write_all(b_arr).await?;
                Ok(())
            }
            Tag::StringTag(string) => write_string(write, string).await,
            Tag::ListTag(tag_type, tags) => {
                write.write_u8(*tag_type).await?;
                write.write_i32(tags.len() as i32).await?;
                for tag in tags {
                    write_tag(tag, write).await?;
                }
                Ok(())
            }
            Tag::CompoundTag(tag) => write_compound_tag(tag, write).await,
            Tag::IntArrayTag(i_arr) => {
                write.write_i32(i_arr.len() as i32).await?;
                for i in i_arr {
                    write.write_i32(*i).await?;
                }
                Ok(())
            }
            Tag::LongArrayTag(l_arr) => {
                write.write_i32(l_arr.len() as i32).await?;
                for l in l_arr {
                    write.write_i64(*l).await?;
                }
                Ok(())
            }
        }
    })
}

fn size_tag(tag: &Tag) -> usize {
    match tag {
        Tag::EndTag => 0,
        Tag::ByteTag(_) => 1,
        Tag::ShortTag(_) => 2,
        Tag::IntTag(_) => 4,
        Tag::LongTag(_) => 8,
        Tag::FloatTag(_) => 4,
        Tag::DoubleTag(_) => 8,
        Tag::ByteArrayTag(b_arr) => b_arr.len() + 4,
        Tag::StringTag(string) => size_string(string),
        Tag::ListTag(_, tags) => {
            let mut size = 5;
            for tag in tags {
                size += size_tag(tag);
            }
            size
        }
        Tag::CompoundTag(tag) => size_compound_tag(tag),
        Tag::IntArrayTag(i_arr) => (i_arr.len() * 4) + 4,
        Tag::LongArrayTag(l_arr) => (l_arr.len() * 8) + 4,
    }
}

fn load_tag<'a, R: AsyncRead + Unpin + ?Sized>(
    tag_bit: u8,
    read: &'a mut R,
    depth: usize,
    accounter: &'a mut NbtAccounter,
) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Tag>> + 'a>> {
    Box::pin(async move {
        match tag_bit {
            0 => {
                accounter.account_bits(64)?;
                Ok(Tag::EndTag)
            }
            1 => {
                accounter.account_bits(72)?;
                Ok(Tag::ByteTag(read.read_u8().await?))
            }
            2 => {
                accounter.account_bits(80)?;
                Ok(Tag::ShortTag(read.read_i16().await?))
            }
            3 => {
                accounter.account_bits(96)?;
                Ok(Tag::IntTag(read.read_i32().await?))
            }
            4 => {
                accounter.account_bits(128)?;
                Ok(Tag::LongTag(read.read_i64().await?))
            }
            5 => {
                accounter.account_bits(96)?;
                Ok(Tag::FloatTag(read.read_f32().await?))
            }
            6 => {
                accounter.account_bits(128)?;
                Ok(Tag::DoubleTag(read.read_f64().await?))
            }
            7 => {
                accounter.account_bits(192)?;
                let size = read.read_i32().await?;
                accounter.account_bits(8 * (size as u64))?;
                let mut bytes = vec![0u8; size as usize];
                read.read_exact(&mut bytes).await?;
                Ok(Tag::ByteArrayTag(bytes))
            }
            8 => {
                accounter.account_bits(288)?;
                let string = read_string(read).await?;
                accounter.account_bits(16 * (string.len() as u64))?;
                Ok(Tag::StringTag(string))
            }
            9 => {
                accounter.account_bits(296)?;
                if depth > 512 {
                    throw_explain!("Nbt tag depth exceeded 512.");
                }

                let list_tag_type = read.read_u8().await?;
                let list_len = read.read_i32().await?;
                if list_tag_type == 0 && list_len > 0 {
                    throw_explain!("Missing type on list tag.");
                }

                accounter.account_bits(32 * (list_len as u64))?;

                let mut tags = Vec::new();
                for _ in 0..list_len {
                    tags.push(load_tag(list_tag_type, read, depth + 1, accounter).await?);
                }
                Ok(Tag::ListTag(list_tag_type, tags))
            }
            10 => {
                accounter.account_bits(384)?;
                if depth > 512 {
                    throw_explain!("Nbt tag depth exceeded 512.");
                }
                let mut mappings = HashMap::new();

                let mut next_byte: u8;
                while {
                    next_byte = read.read_u8().await?;
                    next_byte != 0
                } {
                    let tag_name = read_string(read).await?;
                    accounter.account_bits(224 + (16 * (tag_name.len() as u64)))?;
                    let tag = load_tag(next_byte, read, depth + 1, accounter).await?;
                    if mappings.insert(tag_name, tag).is_some() {
                        accounter.account_bits(288)?;
                    }
                }
                Ok(Tag::CompoundTag(CompoundTag { mappings }))
            }
            11 => {
                accounter.account_bits(192)?;
                let len = read.read_i32().await?;
                accounter.account_bits(32 * (len as u64))?;
                let mut i_arr = vec![0i32; len as usize];
                for _ in 0..len {
                    i_arr.push(read.read_i32().await?);
                }
                Ok(Tag::IntArrayTag(i_arr))
            }
            12 => {
                accounter.account_bits(192)?;
                let len = read.read_i32().await?;
                accounter.account_bits(64 * (len as u64))?;
                let mut l_arr = vec![0i64; len as usize];
                for _ in 0..len {
                    l_arr.push(read.read_i64().await?);
                }
                Ok(Tag::LongArrayTag(l_arr))
            }
            _ => throw_explain!(format!("Unknown tag bit {}", tag_bit)),
        }
    })
}

#[derive(Debug, Default, Clone)]
pub struct CompoundTag {
    mappings: HashMap<String, Tag>,
}

impl CompoundTag {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn put_tag<S: Into<String>>(&mut self, location: S, tag: Tag) {
        self.mappings.insert(location.into(), tag);
    }

    pub fn get_tag(&self, location: &String) -> Option<&Tag> {
        self.mappings.get(location)
    }

    pub fn tags(&self) -> Keys<'_, String, Tag> {
        self.mappings.keys()
    }
}

pub async fn read_nbt<R: AsyncRead + Unpin + ?Sized>(
    read: &mut R,
    limit: u64,
) -> crate::prelude::Result<Option<CompoundTag>> {
    let mut accounter = NbtAccounter { limit, current: 0 };
    let bit = read.read_u8().await?;
    if bit == 0 {
        return Ok(None);
    } else if bit != COMPOUND_TAG_BIT {
        throw_explain!("Root tag must be a compound tag.")
    }
    skip_string(read).await?;
    match load_tag(bit, read, 0, &mut accounter).await? {
        Tag::CompoundTag(tag) => Ok(Some(tag)),
        _ => throw_explain!("Invalid root tag loaded."),
    }
}

pub async fn write_nbt<W: AsyncWrite + Unpin + ?Sized>(
    tag: &CompoundTag,
    writer: &mut W,
) -> crate::prelude::Result<()> {
    writer.write_u8(COMPOUND_TAG_BIT).await?;
    write_string(writer, &String::new()).await?;
    write_compound_tag(tag, writer).await
}

pub async fn write_optional_nbt<W: AsyncWrite + Unpin + ?Sized>(
    tag: &Option<CompoundTag>,
    writer: &mut W,
) -> crate::prelude::Result<()> {
    match tag.as_ref() {
        Some(tag) => {
            writer.write_u8(COMPOUND_TAG_BIT).await?;
            write_string(writer, &String::new()).await?;
            write_compound_tag(tag, writer).await?;
        }
        None => writer.write_all(&[0u8]).await?,
    }
    Ok(())
}

pub fn size_nbt(tag: &CompoundTag) -> usize {
    size_compound_tag(tag)
}

pub fn size_optional_nbt(tag: &Option<CompoundTag>) -> usize {
    match tag.as_ref() {
        Some(tag) => size_compound_tag(tag),
        None => 1,
    }
}

#[macro_export]
macro_rules! ctg {
    ($($name:literal:
        $(($($true_tokens:tt)*))?
        $([$($vec_tokens:tt)*])?
        $({$($ctg_tokens:tt)*})?
        $($v:literal)?
        $($i:ident)?
    ),*) => {
        {
            let mut tag = $crate::nbt::CompoundTag::new();
            $(
                tag.put_tag($name, $crate::nbt::Tag::from(
                    $($v)?
                    $($i)?
                    $($($true_tokens)*)?
                    $($crate::ctg! {$($ctg_tokens)*})?
                    $(vec![$($vec_tokens)*])?
                ));
            )*
            tag
        }
    };
}
