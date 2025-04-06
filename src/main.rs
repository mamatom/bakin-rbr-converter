use std::{u64, usize};
use binrw::prelude::*;
use leb128;
use hex_literal::hex;
use std::io::Cursor;

#[derive(Debug, Clone,Copy)]
struct LEB128(u64);

impl From<LEB128> for usize {
    fn from(value: LEB128) -> Self {
        value.0 as usize
    }
}
impl TryFrom<usize> for LEB128 {
    type Error = std::io::Error;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value > u64::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Value too large for LEB128",
            ));
        }
        Ok(LEB128(value as u64))
    }
}

impl BinRead for LEB128 {
    type Args<'a> = ();
    fn read_options<R: std::io::Read + std::io::Seek>(
            reader: &mut R,
            endian: binrw::Endian,
            args: Self::Args<'_>,
        ) -> BinResult<Self> {
        match leb128::read::unsigned(reader) {
            Ok(value)=> Ok(LEB128(value)),
            Err(e) => return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(e),
            }),
        }
    }
}

impl BinWrite for LEB128 {
    type Args<'a> = ();
    fn write_options<W: std::io::Write + std::io::Seek>(
            &self,
            writer: &mut W,
            endian: binrw::Endian,
            args: Self::Args<'_>,
        ) -> BinResult<()> {
        match leb128::write::unsigned(writer, self.0) {
            Ok(_) => Ok(()),
            Err(e) => return Err(binrw::Error::Custom {
                pos: writer.stream_position().unwrap_or(0),
                err: Box::new(e),
            }),
        }
    }
}

#[binrw]
#[derive(Debug)]
struct SizedString(
    #[br(parse_with = parse_sized_string)]
    #[bw(write_with = write_sized_string)]
    String
);

fn parse_sized_string<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    _args: (),
) -> BinResult<String> {
    // Read LEB128 length
    let length = LEB128::read_options(reader, endian, _args)?;
    
    // Convert to usize and create proper VecArgs
    let usize_length: usize = length.into();
    let bytes = Vec::<u8>::read_args(
        reader, 
        binrw::args! { count: usize_length }
    )?;
    
    String::from_utf8(bytes).map_err(|e| binrw::Error::Custom {
        pos: reader.stream_position().unwrap_or(0),
        err: Box::new(e),
    })
}

fn write_sized_string<W: std::io::Write + std::io::Seek>(
    s: &String,
    writer: &mut W,
    endian: binrw::Endian,  // Now using the endian parameter
    _args: (),
) -> BinResult<()> {
    let length = LEB128(s.len() as u64);
    // Use write_options with explicit endian handling
    length.write_options(writer, endian, ())?;
    writer.write_all(s.as_bytes())?;
    Ok(())
}



fn main() {
    let mut text = Cursor::new(hex!("05 49 74 65 6D 73"));
    let test:SizedString = text.read_le().unwrap();
    println!("String: {:?}", test);

    let mut text2 = Cursor::new(hex!("00 00 00 00 00 00 00 00 00 00 00 00 00"));
    text2.write_le(&SizedString("kettenkrat".to_owned())).unwrap();
    println!("String: {:?}", text2);
}
