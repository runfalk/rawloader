extern crate crossbeam;
use std;
use decoders::NUM_CORES;
use std::mem;

extern crate byteorder;
use self::byteorder::{BigEndian, LittleEndian, ByteOrder};

#[derive(Debug, Copy, Clone)]
pub struct Endian {
  big: bool,
}

impl Endian {
  pub fn ru32(&self, buf: &[u8], pos: usize) -> u32 {
    if self.big {
      BEu32(buf,pos)
    } else {
      LEu32(buf,pos)
    }
  }

  pub fn ru16(&self, buf: &[u8], pos: usize) -> u16 {
    if self.big {
      BEu16(buf,pos)
    } else {
      LEu16(buf,pos)
    }
  }
}


pub static BIG_ENDIAN: Endian = Endian{big: true};
pub static LITTLE_ENDIAN: Endian = Endian{big: false};

#[allow(non_snake_case)] #[inline] pub fn BEu32(buf: &[u8], pos: usize) -> u32 {
  BigEndian::read_u32(&buf[pos..pos+4])
}

#[allow(non_snake_case)] #[inline] pub fn LEu32(buf: &[u8], pos: usize) -> u32 {
  LittleEndian::read_u32(&buf[pos..pos+4])
}

#[allow(non_snake_case)] #[inline] pub fn BEu16(buf: &[u8], pos: usize) -> u16 {
  BigEndian::read_u16(&buf[pos..pos+2])
}

#[allow(non_snake_case)] #[inline] pub fn LEu16(buf: &[u8], pos: usize) -> u16 {
  LittleEndian::read_u16(&buf[pos..pos+2])
}

pub fn decode_12be(buf: &[u8], width: usize, height: usize) -> Vec<u16> {
  decode_threaded(width, height, &(|out: &mut [u16], start, width, _| {
    let inb = &buf[((start*width*12/8) as usize)..];

    for (o, i) in out.chunks_mut(2).zip(inb.chunks(3)) {
      let g1: u16 = i[0] as u16;
      let g2: u16 = i[1] as u16;
      let g3: u16 = i[2] as u16;

      o[0] = (g1 << 4) | (g2 >> 4);
      o[1] = ((g2 & 0x0f) << 8) | g3;
    }
  }))
}

pub fn decode_12le(buf: &[u8], width: usize, height: usize) -> Vec<u16> {
  decode_threaded(width, height, &(|out: &mut [u16], start, width, _| {
    let inb = &buf[((start*width*12/8) as usize)..];

    for (o, i) in out.chunks_mut(2).zip(inb.chunks(3)) {
      let g1: u16 = i[0] as u16;
      let g2: u16 = i[1] as u16;
      let g3: u16 = i[2] as u16;

      o[0] = ((g2 & 0x0f) << 8) | g1;
      o[1] = (g3 << 4) | (g2 >> 4);
    }
  }))
}

pub fn decode_12be_unpacked(buf: &[u8], width: usize, height: usize) -> Vec<u16> {
  decode_threaded(width, height, &(|out: &mut [u16], start, width, _| {
    let inb = &buf[((start*width*2) as usize)..];

    for (o, i) in out.chunks_mut(1).zip(inb.chunks(2)) {
      let g1: u16 = i[0] as u16;
      let g2: u16 = i[1] as u16;

      o[0] = ((g1 & 0x0f) << 8) | g2;
    }
  }))
}

pub fn decode_16le(buf: &[u8], width: usize, height: usize) -> Vec<u16> {
  decode_threaded(width, height, &(|out: &mut [u16], start, width, _| {
    let inb = &buf[((start*width*2) as usize)..];

    for (o, i) in out.chunks_mut(1).zip(inb.chunks(2)) {
      let g1: u16 = i[0] as u16;
      let g2: u16 = i[1] as u16;

      o[0] = (g2 << 8) | g1;
    }
  }))
}

pub fn decode_16be(buf: &[u8], width: usize, height: usize) -> Vec<u16> {
  decode_threaded(width, height, &(|out: &mut [u16], start, width, _| {
    let inb = &buf[((start*width*2) as usize)..];

    for (o, i) in out.chunks_mut(1).zip(inb.chunks(2)) {
      let g1: u16 = i[0] as u16;
      let g2: u16 = i[1] as u16;

      o[0] = (g1 << 8) | g2;
    }
  }))
}

pub fn decode_threaded<F>(width: usize, height: usize, closure: &F) -> Vec<u16>
  where F : Fn(&mut [u16], usize, usize, usize)+std::marker::Sync {
  let mut out: Vec<u16> = vec![0; (width*height) as usize];

  // Default to the number of available physical cores (no hyperthreading)
  let threads = *NUM_CORES;

  // If we're only using one thread do it all sequentially and be done with it
  if threads < 2 || height < threads {
    closure(&mut out, 0, width, height);
    return out
  }

  let mut split = height/threads;
  if split*threads < height { // Make sure the last split is always the smallest
    split += 1;
  }

  crossbeam::scope(|scope| {
    let mut handles = Vec::new();
    for (i,out_part) in (&mut out[..]).chunks_mut(split*width).enumerate() {
      let start = split*(i);
      let tall = out_part.len()/width;
      let handle = scope.spawn(move || {
        closure(out_part, start, width, tall);
      });
      handles.push(handle);
    }

    for h in handles { h.join() };
  });

  out
}

#[derive(Debug, Clone)]
pub struct LookupTable {
  table: Vec<(u16, u16, u16)>,
}

impl LookupTable {
  pub fn new(table: &[u16]) -> LookupTable {
    let mut tbl = vec![(0,0,0); table.len()];
    for i in 0..table.len() {
      let center = table[i];
      let lower = if i > 0 {table[i-1]} else {center};
      let upper = if i < (table.len()-1) {table[i+1]} else {center};
      let base = center - ((upper - lower + 2) / 4);
      let delta = upper - lower;
      tbl[i] = (center, base, delta);
    }
    LookupTable {
      table: tbl,
    }
  }

//  pub fn lookup(&self, value: u16) -> u16 {
//    let (val, _, _) = self.table[value as usize];
//    val
//  }

  pub fn dither(&self, value: u16, rand: &mut u32) -> u16 {
    let (_, sbase, sdelta) = self.table[value as usize];
    let base = sbase as u32;
    let delta = sdelta as u32;
    let pixel = base + ((delta * (*rand & 2047) + 1024) >> 12);
    *rand = 15700 * (*rand & 65535) + (*rand >> 16);
    pixel as u16
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PumpOrder {
  LSB,
  MSB,
}

pub struct BitPump<'a> {
  buffer: &'a [u8],
  pos: usize,
  bits: u64,
  nbits: u32,
  order: PumpOrder,
}

impl<'a> BitPump<'a> {
  pub fn new(src: &'a [u8], order: PumpOrder) -> BitPump {
    BitPump {
      buffer: src,
      pos: 0,
      bits: 0,
      nbits: 0,
      order: order,
    }
  }

  fn fill_bits(&mut self) {
    if self.order == PumpOrder::MSB {
      let inbits: u64 = BEu32(self.buffer, self.pos) as u64;
      self.bits = (self.bits << 32) | inbits;
    } else {
      let inbits: u64 = LEu32(self.buffer, self.pos) as u64;
      self.bits = ((inbits << 32) | (self.bits << (32-self.nbits))) >> (32-self.nbits);
    }
    self.pos += 4;
    self.nbits += 32;
  }

  pub fn peek_bits(&mut self, num: u32) -> u32 {
    if num > self.nbits {
      self.fill_bits();
    }
    if self.order == PumpOrder::MSB {
      (self.bits >> (self.nbits-num)) as u32
    } else {
      (self.bits & (0x0ffffffffu64 >> (32-num))) as u32
    }
  }

  pub fn get_bits(&mut self, num: u32) -> u32 {
    if num == 0 {
      return 0
    }

    let val = self.peek_bits(num);
    self.nbits -= num;
    if self.order == PumpOrder::MSB {
      self.bits &= (1 << self.nbits) - 1;
    } else {
      self.bits >>= num;
    }
    val
  }

  pub fn get_ibits(&mut self, num: u32) -> i32 {
    let val = self.get_bits(num);
    unsafe{mem::transmute(val)}
  }
}
