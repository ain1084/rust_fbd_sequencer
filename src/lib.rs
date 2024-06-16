#![cfg_attr(not(test), no_std)]

use core::{array, cmp};

use arraydeque::ArrayDeque;
#[derive(PartialEq)]
pub enum OutputMode {
    None,
    Tone,
    Noise,
    ToneNoise,
}

pub trait PsgTrait {
    fn sample_rate(&self) -> u32;
    fn clock_rate(&self) -> u32;
    fn set_tone_period(&mut self, channel: usize, period: u16);
    fn set_volume(&mut self, channel: usize, volume: u8);
    fn set_output_mode(&mut self, channel: usize, mode: OutputMode);
    fn set_noise_period(&mut self, period: u8);
    fn next_sample_i16(&mut self) -> i16;
    #[cfg(feature = "float")]
    fn next_sample_f32(&mut self) -> f32;
}

pub trait DataAccessor {
    fn read_byte(&self, index: u16) -> u8;
    fn read_short(&self, index: u16) -> u16;
}

const PART_COUNT: usize = 3;

enum EnvelopePhase {
    Attack,
    Decay,
    Sustain,
    Release,
}

struct Envelope {
    current: u8,
    phase: EnvelopePhase,
    al: u8,
    ar: u8,
    dr: u8,
    sl: u8,
    sr: u8,
    rr: u8,
}

impl Envelope {
    fn new() -> Self {
        Self {
            current: 0,
            phase: EnvelopePhase::Attack,
            al: u8::MAX,
            ar: u8::MAX,
            dr: 0,
            sl: 0,
            sr: 0,
            rr: u8::MAX,
        }
    }

    fn set(
        &mut self,
        patch_number: u8,
        data_accessor: &dyn DataAccessor,
        patch_index: u16,
    ) -> bool {
        let mut index = patch_index;
        loop {
            let l_patch_number = data_accessor.read_byte(index);
            if l_patch_number == patch_number {
                self.al = data_accessor.read_byte(index + 1);
                self.ar = data_accessor.read_byte(index + 2);
                self.dr = data_accessor.read_byte(index + 3);
                self.sl = data_accessor.read_byte(index + 4);
                self.sr = data_accessor.read_byte(index + 5);
                self.rr = data_accessor.read_byte(index + 6);
                break true;
            } else if l_patch_number == 0xFF {
                break false;
            } else {
                index += 7;
            }
        }
    }

    fn attack(&mut self) {
        self.current = self.al;
        self.phase = if self.current != u8::MAX {
            EnvelopePhase::Attack
        } else {
            EnvelopePhase::Decay
        }
    }

    fn release(&mut self) {
        self.phase = EnvelopePhase::Release;
    }

    fn update(&mut self) {
        (self.current, self.phase) = match self.phase {
            EnvelopePhase::Attack => match self.current.checked_add(self.ar) {
                Some(next) => (next, EnvelopePhase::Attack),
                None => (u8::MAX, EnvelopePhase::Decay),
            },
            EnvelopePhase::Decay => {
                let next = self.current.saturating_sub(self.dr);
                if next < self.sl {
                    (self.sl, EnvelopePhase::Sustain)
                } else {
                    (next, EnvelopePhase::Decay)
                }
            }
            EnvelopePhase::Sustain => {
                (self.current.saturating_sub(self.sr), EnvelopePhase::Sustain)
            }
            EnvelopePhase::Release => {
                (self.current.saturating_sub(self.rr), EnvelopePhase::Release)
            }
        }
    }
}

struct PitchLFO {
    displacement: i16,
    delay: u8,
    speed: u8,
    depth: u8,
    is_enable: bool,
    effect: i16,
    current_displacement: i16,
    wait_count: u8,
    depth_count: u8,
}

impl PitchLFO {
    fn new() -> Self {
        Self {
            is_enable: false,
            delay: 0,
            speed: 0,
            depth: 0,
            displacement: 0,
            wait_count: 0,
            depth_count: 0,
            current_displacement: 0,
            effect: 0,
        }
    }

    fn set_enable(&mut self, is_enable: bool) {
        self.is_enable = is_enable;
        self.reset();
    }

    fn set_parameter(&mut self, delay: u8, speed: u8, depth: u8, displacement: i16) {
        self.is_enable = true;
        self.delay = delay;
        self.speed = speed;
        self.depth = depth;
        self.displacement = displacement;
        self.reset();
    }

    fn reset(&mut self) {
        self.wait_count = self.delay;
        self.depth_count = self.depth >> 1;
        self.current_displacement = self.displacement;
        self.effect = 0;
    }

    fn update(&mut self) -> bool {
        if !self.is_enable {
            return false;
        }
        self.wait_count -= 1;
        if self.wait_count != 0 {
            return false;
        }
        self.wait_count = self.speed;
        self.effect += self.current_displacement;
        self.depth_count -= 1;
        if self.depth_count == 0 {
            self.depth_count = self.depth;
            self.current_displacement = -self.current_displacement;
        }
        true
    }
}

struct Repeat {
    start: u16,
    end: Option<u16>,
    count: u8,
}

struct RepeatStack(ArrayDeque<Repeat,8>);

impl RepeatStack {
    fn new() -> Self {
        Self(ArrayDeque::new())
    }

    fn start(&mut self, count: u8, current_index: u16) {
        let _ = self.0.push_front(Repeat {
            count,
            start: current_index,
            end: None,
        });
    }

    fn break_if_last(&mut self, current_index: &mut u16) {
        if let Some(item) = self.0.front() {
            if item.count == 1 {
                if let Some(end) = item.end {
                    *current_index = end;
                    self.0.pop_front();
                }
            }
        }
    }

    fn end(&mut self, current_index: &mut u16) -> bool {
        if let Some(item) = self.0.front_mut() {
            let is_infinite_loop = if item.count == 0 {
                true
            } else {
                item.count -= 1;
                false
            };
            if is_infinite_loop || item.count != 0 {
                (*item).end = Some(*current_index);
                *current_index = item.start;
            } else {
                self.0.pop_front();
            }
            is_infinite_loop
        } else {
            false
        }
    }
}

struct Part<'a> {
    data_accessor: &'a dyn DataAccessor,
    patch_index: u16,
    envelope: Envelope,
    repeats: RepeatStack,
    pitch_lfo: PitchLFO,
    channel_number: usize,
    next_index: u16,
    length: u8,
    is_tie: bool,
    is_end: bool,
    octave: u8,
    volume: u8,
    tone_period: u16,
    detune: i16,
    infinite_loop_count: u16,
}

impl<'a> Part<'a> {
    fn new(
        data_accessor: &'a dyn DataAccessor,
        patch_index: u16,
        channel_number: usize,
        next_index: u16,
    ) -> Self {
        Self {
            data_accessor,
            patch_index,
            envelope: Envelope::new(),
            pitch_lfo: PitchLFO::new(),
            repeats: RepeatStack::new(),
            channel_number,
            next_index,
            length: 1,
            is_tie: false,
            is_end: false,
            octave: 0,
            volume: 0,
            tone_period: 0,
            detune: 0,
            infinite_loop_count: 0,
        }
    }

    fn split_tone_period_and_octave(note: u8) -> (u16, u8) {
        const TONE_PERIOD_VALUES: &'static [u16] = &[
            3816, 3602, 3400, 3209, 3029, 2859, 2698, 2547, 2404, 2269, 2142, 2022,
        ];
        (TONE_PERIOD_VALUES[(note % 12) as usize], note / 12)
    }

    fn next_byte(&mut self) -> u8 {
        let result = self.data_accessor.read_byte(self.next_index);
        self.next_index += 1;
        return result;
    }

    fn next_signed_short(&mut self) -> i16 {
        let result = self.data_accessor.read_short(self.next_index) as i16;
        self.next_index += 2;
        return result;
    }

    fn update_volume(&mut self, psg: &mut dyn PsgTrait) {
        self.envelope.update();
        self.apply_volume(psg);
    }

    fn apply_volume(&self, psg: &mut dyn PsgTrait) {
        psg.set_volume(
            self.channel_number,
            ((self.envelope.current as u16 * self.volume as u16) >> 8) as u8,
        );
    }

    fn update_tone_period(&mut self, psg: &mut dyn PsgTrait) {
        if self.pitch_lfo.update() {
            self.apply_tone_period(psg);
        }
    }

    fn apply_tone_period(&self, psg: &mut dyn PsgTrait) {
        psg.set_tone_period(
            self.channel_number,
            cmp::min(
                cmp::max(
                    (self.tone_period as i16 + self.detune + self.pitch_lfo.effect) >> self.octave,
                    1,
                ),
                4095,
            ) as u16,
        );
    }

    fn end(&mut self, psg: &mut dyn PsgTrait) {
        psg.set_volume(self.channel_number, 0);
        self.is_end = true
    }

    fn tick(&mut self, psg: &mut dyn PsgTrait) -> bool {
        if self.is_end {
            return false;
        }
        self.length -= 1;
        if self.length != 0 {
            self.update_tone_period(psg);
            self.update_volume(psg);
            return true;
        }
        loop {
            let data = self.next_byte();
            match data {
                0..=0x7f => {
                    if !self.is_tie {
                        self.envelope.release();
                    }
                    self.length = data + 1;
                    break true;
                }
                0x80..=0xDF => {
                    (self.tone_period, self.octave) = Part::split_tone_period_and_octave(data - 0x80_u8);
                    if !self.is_tie {
                        self.envelope.attack();
                        self.pitch_lfo.reset();
                    }
                    self.length = self.next_byte();
                    self.is_tie = if self.data_accessor.read_byte(self.next_index) == 0xE8 {
                        self.next_index += 1;
                        true
                    } else {
                        false
                    };
                    self.apply_tone_period(psg);
                    self.apply_volume(psg);
                    break true;
                }
                0xE0 => {
                    let patch_number = self.next_byte();
                    self.envelope
                        .set(patch_number, self.data_accessor, self.patch_index);
                }
                0xE1 => self.volume = self.next_byte(),
                0xE2 => {
                    let count = self.next_byte();
                    self.repeats.start(count, self.next_index);
                }
                0xE3 => self.repeats.break_if_last(&mut self.next_index),
                0xE4 => {
                    let detect_infinite_loop = self.repeats.end(&mut self.next_index);
                    if detect_infinite_loop {
                        self.infinite_loop_count = self.infinite_loop_count.saturating_add(1);
                    }
                }
                0xE5 => {
                    psg.set_noise_period(self.next_byte());
                }
                0xE6 => self.volume = cmp::min(self.volume + 1, 15),
                0xE7 => self.volume = self.volume.saturating_sub(1),
                0xE9 => {
                    self.detune = self.next_signed_short();
                }
                0xEA => {
                    let delay = self.next_byte();
                    let speed = self.next_byte();
                    let depth = self.next_byte();
                    let displacement = self.next_signed_short();
                    self.pitch_lfo.set_parameter(delay, speed, depth, displacement);
                }
                0xEB => {
                    let data = self.next_byte();
                    self.pitch_lfo.set_enable(data != 0);
                }
                0xEC => {
                    let mode = match self.next_byte() {
                        0x01 => OutputMode::Tone,
                        0x02 => OutputMode::Noise,
                        0x03 => OutputMode::ToneNoise,
                        _ => OutputMode::None,
                    };
                    psg.set_output_mode(self.channel_number, mode);
                }
                _ => {
                    self.end(psg);
                    break false;
                }
            }
        }
    }
}

const INTERVAL_RATIO_X100: u32 = 5994;
struct SamplesPerTick {
    remainder: u32,
    quotient: u32,
    error: i32,
    samples: usize,
}

impl SamplesPerTick {
    fn new(sample_rate: u32) -> Self {
        let sample_rate_x100 = sample_rate * 100;
        let mut instance = Self {
            quotient: sample_rate_x100 / INTERVAL_RATIO_X100,
            remainder: sample_rate_x100 % INTERVAL_RATIO_X100,
            error: -(INTERVAL_RATIO_X100 as i32),
            samples: 0,
        };
        instance.next();
        instance
    }

    fn samples(&self) -> usize {
        self.samples
    }

    fn consume<'a>(&mut self, samples: usize) -> bool {
        self.samples -= samples;
        self.samples != 0
    }

    fn next(&mut self) {
        self.error += self.remainder as i32;
        self.samples = (self.quotient
            + if self.error >= 0 {
                self.error -= INTERVAL_RATIO_X100 as i32;
                1
            } else {
                0
            }) as usize;
    }
}

pub struct PlayContext<'a> {
    parts: [Option<Part<'a>>; PART_COUNT],
    psg: &'a mut dyn PsgTrait,
    samples_per_tick: SamplesPerTick,
    max_loop_count: Option<usize>,
}

impl<'a> PlayContext<'a> {
    fn new(parts: [Option<Part<'a>>; PART_COUNT], psg: &'a mut dyn PsgTrait) -> Self {
        let sample_rate = psg.sample_rate();
        for channel in 0..PART_COUNT {
            psg.set_output_mode(channel, OutputMode::Tone);
            psg.set_volume(channel, 0);
            psg.set_tone_period(channel, 0);
        }
        psg.set_noise_period(0);
        Self {
            parts,
            psg,
            samples_per_tick: SamplesPerTick::new(sample_rate),
            max_loop_count: None,
        }
    }

    pub fn set_max_loop_count(&mut self, count: Option<usize>) {
        self.max_loop_count = count;
        self.apply_max_loop_count();
    }

    fn next_sample_internal<T>(
        &mut self,
        buffer: &mut [T],
        mut f: impl FnMut(&mut dyn PsgTrait) -> T,
    ) -> usize {
        let mut buffer_len = buffer.len();
        let mut buffer_index: usize = 0;
        while buffer_len != 0 {
            let fill_len = cmp::min(self.samples_per_tick.samples(), buffer_len);
            buffer[buffer_index..buffer_index + fill_len].fill_with(|| f(self.psg));
            buffer_index += fill_len;
            buffer_len -= fill_len;
            if !self.samples_per_tick.consume(fill_len) {
                if self.apply_max_loop_count() {
                    break;
                }
                if !self.tick() {
                    break;
                }
                self.samples_per_tick.next();
            }
        }
        buffer_index
    }

    pub fn next_samples_i16(&mut self, buffer: &mut [i16]) -> usize {
        self.next_sample_internal(buffer, |psg| psg.next_sample_i16())
    }

    #[cfg(feature = "float")]
    pub fn next_samples_f32(&mut self, buffer: &mut [f32]) -> usize {
        self.next_sample_internal(buffer, |psg| psg.next_sample_f32())
    }

    pub fn is_playing(&self) -> bool {
        self.parts.iter().any(|o_part| o_part.is_some())
    }

    pub fn tick(&mut self) -> bool {
        let mut playing = false;
        self.parts.iter_mut().for_each(|o_part| {
            if let Some(part) = o_part {
                if part.tick(self.psg) {
                    playing = true
                } else {
                    *o_part = None
                }
            }
        });
        playing
    }

    pub fn end(&mut self) {
        self.parts.iter_mut().for_each(|o_part| {
            if let Some(part) = o_part {
                part.end(self.psg);
            }
        })
    }

    fn apply_max_loop_count(&mut self) -> bool {
        if let Some(count) = self.max_loop_count {
            if self.infinite_loop_count() as usize >= count {
                self.end();
                return true;
            }
        }
        false
    }

    fn infinite_loop_count(&self) -> u16 {
        self.parts
            .iter()
            .filter_map(|o_part| {
                if let Some(part) = o_part {
                    return Some(part.infinite_loop_count);
                } else {
                    None
                }
            })
            .max()
            .unwrap_or_default()
    }
}

pub struct TitleIterator<'a> {
    data_accessor: &'a dyn DataAccessor,
    index: u16
}

impl<'a> Iterator for TitleIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let ch = self.data_accessor.read_byte(self.index);
        if ch == 0 {
            None
        } else {
            self.index += 1;
            Some(if ch == b'\n' { b' '} else { ch })
        }
    }
}

pub struct Sequencer<'a> {
    data_accessor: &'a dyn DataAccessor,
    patch_index: u16,
    part_indexes: [Option<u16>; PART_COUNT],
}

impl<'a> Sequencer<'a> {
    pub fn new(data_accessor: &'a dyn DataAccessor) -> Self {
        let mut index = 0;
        loop {
            if data_accessor.read_byte(index) == 0 {
                break;
            }
            index += 1;
        }
        let body_index_offset = index;
        index += 2;
        let patch_index = data_accessor.read_short(index) as u16 + body_index_offset;
        index += 2;
        Self {
            data_accessor,
            patch_index,
            part_indexes: array::from_fn(|i| {
                let part_index_offset = data_accessor.read_short(index + i as u16 * 2) as u16;
                match part_index_offset {
                    0 => None,
                    _ => Some(part_index_offset + body_index_offset),
                }
            }),
        }
    }

    pub fn title_iter(&self) -> TitleIterator {
        TitleIterator {
            data_accessor: self.data_accessor,
            index: 0
        }
    }

    pub fn play(&self, psg: &'a mut dyn PsgTrait) -> PlayContext<'a> {
        PlayContext::new(
            array::from_fn(|part_number| match self.part_indexes[part_number] {
                Some(part_index) => Some(Part::new(
                    self.data_accessor,
                    self.patch_index,
                    part_number,
                    part_index,
                )),
                None => None,
            }),
            psg,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::{ByteOrder, LittleEndian};

    struct DummyPsg {}

    impl PsgTrait for DummyPsg {
        fn sample_rate(&self) -> u32 {
            44100
        }
        fn clock_rate(&self) -> u32 {
            2_000_000
        }
        fn set_tone_period(&mut self, _channel: usize, _tune: u16) {}
        fn set_volume(&mut self, _channel: usize, _volume: u8) {}
        fn set_output_mode(&mut self, _channel: usize, _mode: OutputMode) {}
        fn set_noise_period(&mut self, _frequency: u8) {}
        fn next_sample_i16(&mut self) -> i16 {
            0i16
        }
        #[cfg(feature = "float")]
        fn next_sample_f32(&mut self) -> f32 {
            0.0f32
        }
    }

    impl<const N: usize> DataAccessor for [u8; N] {
        fn read_byte(&self, index: u16) -> u8 {
            self[index as usize]
        }
        fn read_short(&self, index: u16) -> u16 {
            LittleEndian::read_u16(&self[index as usize..])
        }
    }

    struct TestContext<'a> {
        sequencer: Sequencer<'a>,
        sg: DummyPsg,
    }

    impl<'a> TestContext<'a> {
        fn new(data_accessor: &'a dyn DataAccessor) -> Self {
            Self {
                sequencer: Sequencer::new(data_accessor),
                sg: DummyPsg {},
            }
        }
        fn create_player(&'a mut self) -> PlayContext<'a> {
            self.sequencer.play(&mut self.sg)
        }
    }

    #[test]
    fn test_data_accessor() {
        let data: [u8; 4] = [0x00, 0x01, 0x02, 0x03];
        assert_eq!(data.read_byte(0), 0x00);
        assert_eq!(data.read_byte(1), 0x01);
        assert_eq!(data.read_byte(2), 0x02);
        assert_eq!(data.read_short(0), 0x0100);
        assert_eq!(data.read_short(1), 0x0201);
        assert_eq!(data.read_short(2), 0x0302);
    }
    #[test]
    fn test_header() {
        #[rustfmt::skip]
        const DATA: [u8; 13] = [
            0x41, 0x42, 0x43,
            0x00, // title end
            0x00, // flags (unused)
            0x12, 0x34, // patch offset
            0x56, 0x78, // part 0 offset
            0x9a, 0xbc, // part 1 offset
            0x00, 0x00, // part 2 offset
        ];
        let context = TestContext::new(&DATA);
        let sequencer = &context.sequencer;
        let title = String::from_utf8(sequencer.title_iter().collect::<Vec<u8>>()).unwrap();
        assert_eq!(title, "ABC");
        assert_eq!(sequencer.patch_index, 0x3412 + 3);
        assert_eq!(sequencer.part_indexes[0].unwrap(), 0x7856 + 3);
        assert_eq!(sequencer.part_indexes[1].unwrap(), 0xbc9a + 3);
        assert!(sequencer.part_indexes[2].is_none());
    }

    #[test]
    fn test_sequencer() {
        const DATA: [u8; 12] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x0b, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            0x10, // part 0 body
            0x20, // part 1 body
        ];
        let mut context = TestContext::new(&DATA);
        let player = context.create_player();
        assert!(player.is_playing());

        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.channel_number, 0);
        assert_eq!(part.next_index, 0x000a);

        let part = player.parts[1].as_ref().unwrap();
        assert_eq!(part.channel_number, 1);
        assert_eq!(part.next_index, 0x000b);

        assert!(player.parts[2].is_none());
    }

    #[test]
    fn test_part_next_data() {
        const DATA: [u8; 15] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            0x10, // part 0 body
            0xff, 0x7f, // short 32767
            0x00, 0xff, // short -256
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        let part = player.parts[0].as_mut().unwrap();
        assert_eq!(part.next_byte(), 0x10u8);
        assert_eq!(part.next_signed_short(), 32767i16);
        assert_eq!(part.next_signed_short(), -256i16);
    }

    #[test]
    fn test_part_command_reset() {
        const DATA: [u8; 13] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            // part 0 body
            0x00, // reset 1 tick
            0x01, // reset 2 ticks
            0xff, // end
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        // first dummy tick
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0a);
        assert!(player.tick());

        // 0x00 (1 tick reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0b);
        assert!(player.tick());

        // 0x01 (2 ticks reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 2);
        assert_eq!(part.next_index, 0x0c);
        assert!(player.tick());

        // 0x01 (continue)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0c);
        assert!(!player.tick());

        assert!(player.parts[0].is_none());

        assert!(!player.is_playing());
    }

    #[test]
    fn test_part_commands() {
        const DATA: [u8; 19] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            // part 0 body
            0xE1, 0x08, // volume 8
            0x80, 0x01, // o1c 1 tick
            0xE1, 0x0f, // volume 15
            0x8d, 0x02, // o2c+ 2 ticks
            0xff, // end
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        // first dummy tick
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0a);
        assert!(player.tick());

        // 0xE1, 0x08 volume 8
        // 0x80, 0x01 (1 tick o1c)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.octave, 0);
        assert_eq!(part.volume, 8);
        assert_eq!(part.next_index, 0x0e);
        assert!(player.tick());

        // 0xE1, 0x08 volume 15
        // 0x8d, 0x02 (2 ticks o2d+)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 2);
        assert_eq!(part.octave, 1);
        assert_eq!(part.volume, 15);
        assert_eq!(part.next_index, 0x12);
        assert!(player.tick());

        // 0x8d, 0x02 (continue)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x12);
        assert!(!player.tick());

        assert!(player.parts[0].is_none());

        assert_eq!(player.is_playing(), false);
    }

    #[test]
    fn test_part_command_repeat() {
        const DATA: [u8; 18] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            // part 0 body
            0xE2, 0x02, // repeat start count 2
            0x00, // reset 1 tick
            0xE3, // break loop if count = 1
            0x00, // reset 1 tick
            0xE4, // repeat end
            0x00, // reset 1 clock
            0xff, // end
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        // first dummy tick
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0a);
        assert!(player.tick());

        // 0xE2 0x02 (repeat start count 2)
        // 0x00 (1 tick reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0d);
        assert_eq!(part.repeats.0.len(), 1);
        assert_eq!(part.repeats.0.front().unwrap().count, 2);
        assert!(player.tick());

        // 0xE3 (break loop if count = 1)
        // 0x00 (1 tick reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0f);
        assert_eq!(part.repeats.0.len(), 1);
        assert_eq!(part.repeats.0.front().unwrap().count, 2);
        assert!(player.tick());

        // 0x00 (1 tick reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x0d);
        assert_eq!(part.repeats.0.len(), 1);
        assert_eq!(part.repeats.0.front().unwrap().count, 1);
        assert!(player.tick());

        // repeat end
        // 0x00 (1 tick reset)
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.next_index, 0x11);
        assert_eq!(part.repeats.0.len(), 0);
        assert!(!player.tick());

        assert!(player.parts[0].is_none());
        assert!(!player.is_playing());
    }

    #[test]
    fn test_patch() {
        const DATA: [u8; 22] = [
            0x00, // title end
            0x00, // flags (unused)
            0x0a, 0x00, // patch offset
            0x12, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0xFF, // patch: 0x01
            0xE0, 0x01, 0x00, // part 0 body
            0xFF,
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        // default patch
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.envelope.al, 0xFF);
        assert_eq!(part.envelope.ar, 0xFF);
        assert_eq!(part.envelope.dr, 0x00);
        assert_eq!(part.envelope.sl, 0x00);
        assert_eq!(part.envelope.sr, 0x00);
        assert_eq!(part.envelope.rr, 0xFF);
        assert!(player.tick());

        // patch: 0x01
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.envelope.al, 0x02);
        assert_eq!(part.envelope.ar, 0x03);
        assert_eq!(part.envelope.dr, 0x04);
        assert_eq!(part.envelope.sl, 0x05);
        assert_eq!(part.envelope.sr, 0x06);
        assert_eq!(part.envelope.rr, 0x07);
    }

    #[test]
    fn test_part() {
        const DATA: [u8; 12] = [
            0x00, // title end
            0x00, // flags (unused)
            0x00, 0x00, // patch offset
            0x0a, 0x00, // part 0 offset
            0x0a, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            0xFF, // part 0 body
            0xFF, // part 1 body
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        assert!(player.parts[0].is_some());
        assert_eq!(player.parts[0].as_ref().unwrap().length, 1);
        assert!(player.parts[1].as_ref().is_some());
        assert_eq!(player.parts[1].as_ref().unwrap().length, 1);
        assert!(player.parts[2].as_ref().is_none());
        assert!(player.is_playing());
        assert!(!player.tick());
        assert!(player.parts[0].is_none());
        assert!(player.parts[1].is_none());
        assert!(player.parts[2].is_none());
        assert!(!player.is_playing());
    }

    #[test]
    fn test_part_patch() {
        #[rustfmt::skip]
        const DATA: [u8; 34] = [
            0x00, // title end
            0x00, // flags (unused)
            0x0a, 0x00, // patch offset
            0x19, 0x00, // part 0 offset
            0x00, 0x00, // part 1 offset
            0x00, 0x00, // part 2 offset
            // patch 0 (al = 0x10, ar = 0x10, dr = 0xFF, sr = 0xFF, sl = 0xFF, rr = 0x01)
            0x00, 0x10, 0x10, 0xFF, 0xFF, 0xFF, 0x01,
            // patch 1 (al = 0x20, ar = 0x20, dr = 0xFF, sr = 0xFF, sl = 0xFF, rr = 0x01)
            0x01, 0x20, 0x10, 0xFF, 0xFF, 0xFF, 0x01,
            // patch table end
            0xFF,
            // part 0 body (patch 0x00, o1c 1 clock)
            0xE0, 0x00, 0x80, 0x01,
            // part 0 body (patch 0x01, o1c 1 clock)
            0xE0, 0x01, 0x80, 0x02,
            0xFF,
        ];
        let mut context = TestContext::new(&DATA);
        let mut player = context.create_player();
        assert!(player.is_playing());

        assert!(player.parts[0].as_ref().is_some());
        assert!(player.tick());

        assert!(player.parts[0].as_ref().is_some());
        assert!(player.is_playing());
        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.envelope.current, 0x10);
        assert!(player.tick());

        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 2);
        assert_eq!(part.envelope.current, 0x20);
        assert!(player.tick());

        let part = player.parts[0].as_ref().unwrap();
        assert_eq!(part.length, 1);
        assert_eq!(part.envelope.current, 0x30);
        assert!(!player.tick());
    }
}
