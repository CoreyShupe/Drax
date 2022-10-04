use std::{
    collections::HashMap,
    io::{Read, Write},
};

use crate::transport::{Error, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

pub const COMPOUND_TAG_BIT: u8 = 10;

pub struct NbtAccounter {
    limit: u64,
    current: u64,
}

impl NbtAccounter {
    fn account_bits(&mut self, bits: u64) -> Result<()> {
        match self.current.checked_add(bits) {
            Some(next) => {
                if next > self.limit {
                    return Error::cause(format!(
                        "Nbt tag too big, read {} bytes of allowed {}.",
                        next, self.limit
                    ));
                }
                self.current = next;
                Ok(())
            }
            None => Error::cause("Overflowed bits in accounter."),
        }
    }
}

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

fn skip_bytes<R: Read, I: Into<u64>>(read: &mut R, i: I) -> Result<()> {
    std::io::copy(&mut read.take(i.into()), &mut std::io::sink())
        .map_err(Error::TokioError)
        .map(|_| ())
}

fn skip_string<R: Read>(read: &mut R) -> Result<()> {
    let skipped = read.read_u16::<BigEndian>().map_err(Error::TokioError)?;
    skip_bytes(read, skipped)?;
    Ok(())
}

fn read_string<R: Read>(read: &mut R) -> Result<String> {
    let str_len = read.read_u16::<BigEndian>().map_err(Error::TokioError)?;
    if str_len == 0 {
        return Ok(String::new());
    }
    let mut bytes = vec![0u8; str_len as usize];
    let mut bytes_read = 0;
    while bytes_read < bytes.len() {
        match read
            .read(&mut bytes[bytes_read..])
            .map_err(Error::TokioError)?
        {
            0 => return Error::cause("Invalid NBT string, under read."),
            n => bytes_read += n,
        }
    }
    cesu8::from_java_cesu8(&bytes)
        .map_err(|err| Error::Unknown(Some(format!("Cesu8 encoding error: {}", err))))
        .map(|cow| cow.to_string())
}

fn size_string(string: &str) -> usize {
    4 + cesu8::to_java_cesu8(string).len()
}

fn write_string<W: Write>(write: &mut W, string: &String) -> Result<()> {
    write
        .write_u16::<BigEndian>(string.len() as u16)
        .map_err(Error::TokioError)?;
    write
        .write_all(&cesu8::to_java_cesu8(string))
        .map_err(Error::TokioError)?;
    Ok(())
}

fn write_compound_tag<W: Write>(tag: &CompoundTag, write: &mut W) -> Result<()> {
    for (key, value) in &tag.mappings {
        let id = value.get_bit();
        write.write_u8(id).map_err(Error::TokioError)?;
        if id == 0 {
            return Ok(());
        }
        write_string(write, key)?;
        write_tag(value, write)?;
    }
    write.write_u8(0).map_err(Error::TokioError)
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

fn write_tag<W: Write>(tag: &Tag, write: &mut W) -> Result<()> {
    match tag {
        Tag::EndTag => Ok(()),
        Tag::ByteTag(byte) => write.write_u8(*byte).map_err(Error::TokioError),
        Tag::ShortTag(short) => write
            .write_i16::<BigEndian>(*short)
            .map_err(Error::TokioError),
        Tag::IntTag(int) => write
            .write_i32::<BigEndian>(*int)
            .map_err(Error::TokioError),
        Tag::LongTag(long) => write
            .write_i64::<BigEndian>(*long)
            .map_err(Error::TokioError),
        Tag::FloatTag(float) => write
            .write_f32::<BigEndian>(*float)
            .map_err(Error::TokioError),
        Tag::DoubleTag(double) => write
            .write_f64::<BigEndian>(*double)
            .map_err(Error::TokioError),
        Tag::ByteArrayTag(b_arr) => {
            write
                .write_i32::<BigEndian>(b_arr.len() as i32)
                .map_err(Error::TokioError)?;
            write.write_all(b_arr).map_err(Error::TokioError)?;
            Ok(())
        }
        Tag::StringTag(string) => write_string(write, string),
        Tag::ListTag(tag_type, tags) => {
            write.write_u8(*tag_type).map_err(Error::TokioError)?;
            write
                .write_i32::<BigEndian>(tags.len() as i32)
                .map_err(Error::TokioError)?;
            for tag in tags {
                write_tag(tag, write)?;
            }
            Ok(())
        }
        Tag::CompoundTag(tag) => write_compound_tag(tag, write),
        Tag::IntArrayTag(i_arr) => {
            write
                .write_i32::<BigEndian>(i_arr.len() as i32)
                .map_err(Error::TokioError)?;
            for i in i_arr {
                write
                    .write_i32::<BigEndian>(*i)
                    .map_err(Error::TokioError)?;
            }
            Ok(())
        }
        Tag::LongArrayTag(l_arr) => {
            write
                .write_i32::<BigEndian>(l_arr.len() as i32)
                .map_err(Error::TokioError)?;
            for l in l_arr {
                write
                    .write_i64::<BigEndian>(*l)
                    .map_err(Error::TokioError)?;
            }
            Ok(())
        }
    }
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

fn load_tag<R: Read>(
    tag_bit: u8,
    read: &mut R,
    depth: usize,
    accounter: &mut NbtAccounter,
) -> Result<Tag> {
    match tag_bit {
        0 => {
            accounter.account_bits(64)?;
            Ok(Tag::EndTag)
        }
        1 => {
            accounter.account_bits(72)?;
            Ok(Tag::ByteTag(read.read_u8().map_err(Error::TokioError)?))
        }
        2 => {
            accounter.account_bits(80)?;
            Ok(Tag::ShortTag(
                read.read_i16::<BigEndian>().map_err(Error::TokioError)?,
            ))
        }
        3 => {
            accounter.account_bits(96)?;
            Ok(Tag::IntTag(
                read.read_i32::<BigEndian>().map_err(Error::TokioError)?,
            ))
        }
        4 => {
            accounter.account_bits(128)?;
            Ok(Tag::LongTag(
                read.read_i64::<BigEndian>().map_err(Error::TokioError)?,
            ))
        }
        5 => {
            accounter.account_bits(96)?;
            Ok(Tag::FloatTag(
                read.read_f32::<BigEndian>().map_err(Error::TokioError)?,
            ))
        }
        6 => {
            accounter.account_bits(128)?;
            Ok(Tag::DoubleTag(
                read.read_f64::<BigEndian>().map_err(Error::TokioError)?,
            ))
        }
        7 => {
            accounter.account_bits(192)?;
            let size = read.read_i32::<BigEndian>().map_err(Error::TokioError)?;
            accounter.account_bits(8 * (size as u64))?;
            let mut bytes = vec![0u8; size as usize];
            read.read_exact(&mut bytes).map_err(Error::TokioError)?;
            Ok(Tag::ByteArrayTag(bytes))
        }
        8 => {
            accounter.account_bits(288)?;
            let string = read_string(read)?;
            accounter.account_bits(16 * (string.len() as u64))?;
            Ok(Tag::StringTag(string))
        }
        9 => {
            accounter.account_bits(296)?;
            if depth > 512 {
                return Error::cause("Nbt tag depth exceeded 512.");
            }

            let list_tag_type = read.read_u8().map_err(Error::TokioError)?;
            let list_len = read.read_i32::<BigEndian>().map_err(Error::TokioError)?;
            if list_tag_type == 0 && list_len > 0 {
                return Error::cause("Missing type on list tag.");
            }

            accounter.account_bits(32 * (list_len as u64))?;

            let mut tags = Vec::new();
            for _ in 0..list_len {
                tags.push(load_tag(list_tag_type, read, depth + 1, accounter)?);
            }
            Ok(Tag::ListTag(list_tag_type, tags))
        }
        10 => {
            accounter.account_bits(384)?;
            if depth > 512 {
                return Error::cause("Nbt tag depth exceeded 512.");
            }
            let mut mappings = HashMap::new();

            let mut next_byte: u8;
            while {
                next_byte = read.read_u8().map_err(Error::TokioError)?;
                next_byte != 0
            } {
                let tag_name = read_string(read)?;
                accounter.account_bits(224 + (16 * (tag_name.len() as u64)))?;
                let tag = load_tag(next_byte, read, depth + 1, accounter)?;
                if mappings.insert(tag_name, tag).is_some() {
                    accounter.account_bits(288)?;
                }
            }
            Ok(Tag::CompoundTag(CompoundTag { mappings }))
        }
        11 => {
            accounter.account_bits(192)?;
            let len = read.read_i32::<BigEndian>().map_err(Error::TokioError)?;
            accounter.account_bits(32 * (len as u64))?;
            let mut i_arr = vec![0i32; len as usize];
            for _ in 0..len {
                i_arr.push(read.read_i32::<BigEndian>().map_err(Error::TokioError)?);
            }
            Ok(Tag::IntArrayTag(i_arr))
        }
        12 => {
            accounter.account_bits(192)?;
            let len = read.read_i32::<BigEndian>().map_err(Error::TokioError)?;
            accounter.account_bits(64 * (len as u64))?;
            let mut l_arr = vec![0i64; len as usize];
            for _ in 0..len {
                l_arr.push(read.read_i64::<BigEndian>().map_err(Error::TokioError)?);
            }
            Ok(Tag::LongArrayTag(l_arr))
        }
        _ => Error::cause(format!("Unknown tag bit {}", tag_bit)),
    }
}

pub struct CompoundTag {
    mappings: HashMap<String, Tag>,
}

impl CompoundTag {
    pub fn put_tag(&mut self, location: String, tag: Tag) {
        self.mappings.insert(location, tag);
    }

    pub fn get_tag(&self, location: &String) -> Option<&Tag> {
        self.mappings.get(location)
    }
}

pub fn read_nbt<R: Read>(
    read: &mut R,
    limit: u64,
) -> crate::transport::Result<Option<CompoundTag>> {
    let mut accounter = NbtAccounter { limit, current: 0 };
    let bit = read.read_u8().map_err(Error::TokioError)?;
    if bit == 0 {
        return Ok(None);
    } else if bit != COMPOUND_TAG_BIT {
        return Error::cause("Root tag must be a compound tag.");
    }
    skip_string(read)?;
    match load_tag(bit, read, 0, &mut accounter)? {
        Tag::CompoundTag(tag) => Ok(Some(tag)),
        _ => Error::cause("Invalid root tag loaded."),
    }
}

pub fn write_nbt<W: Write>(tag: &CompoundTag, writer: &mut W) -> Result<()> {
    writer
        .write_u8(COMPOUND_TAG_BIT)
        .map_err(Error::TokioError)?;
    write_string(writer, &String::new())?;
    write_compound_tag(tag, writer)
}

pub fn write_optional_nbt<W: Write>(tag: &Option<CompoundTag>, writer: &mut W) -> Result<()> {
    match tag.as_ref() {
        Some(tag) => {
            writer
                .write_u8(COMPOUND_TAG_BIT)
                .map_err(Error::TokioError)?;
            write_string(writer, &String::new())?;
            write_compound_tag(tag, writer)
        }
        None => writer.write_all(&[0u8]).map_err(Error::TokioError),
    }
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
