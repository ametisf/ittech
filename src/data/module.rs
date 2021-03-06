use super::*;


#[derive(Clone, Debug)]
pub struct Module {
    /// Song Name, null-terminated (but may also contain nulls)
    pub name: Name,

    /// Comment message
    pub message: String,

    /// Rows per Measure highlight, Rows per Beat highlight
    pub highlight: (u8, u8),

    /// "Made With" Tracker
    pub made_with_version: u16,

    /// "Compatible With" Tracker
    pub compatible_with_version: u16,

    /// combined Header Flags and Special Flags, for embedding extra information
    pub flags: ModuleFlags,

    /// Global Volume (0...128)
    pub global_volume: RangedU8<0, 128>,

    /// Sample Volume (0...128)
    pub sample_volume: RangedU8<0, 128>,

    /// Initial Speed (1...255)
    pub speed: RangedU8<1, 255>,

    /// Initial Tempo (31...255)
    pub tempo: RangedU8<31, 255>,

    /// Pan Separation (0...128)
    pub pan_separation: RangedU8<0, 128>,

    /// Pitch Wheel Depth
    pub pitch_wheel_depth: u8,

    /// Initial Channel Panning
    pub init_channel_panning: [u8; 64],

    /// Initial Channel Volume
    pub init_channel_volume: [u8; 64],

    /// Orders
    pub orders: Vec<Order>,

    /// Instrument headers (without samples)
    pub instruments: Vec<Instrument>,

    /// Samples
    pub samples: Vec<Sample>,

    /// Patterns
    pub patterns: Vec<Pattern>,
}

pub(crate) struct ModuleHeader {
    pub(crate) name: Name,
    pub(crate) highlight: (u8, u8),
    pub(crate) made_with_version: u16,
    pub(crate) compatible_with_version: u16,
    pub(crate) flags: ModuleFlags,
    pub(crate) global_volume: RangedU8<0, 128>,
    pub(crate) sample_volume: RangedU8<0, 128>,
    pub(crate) speed: RangedU8<1, 255>,
    pub(crate) tempo: RangedU8<31, 255>,
    pub(crate) pan_separation: RangedU8<0, 128>,
    pub(crate) pitch_wheel_depth: u8,
    pub(crate) message_length: u16,
    pub(crate) message_offset: u32,
    pub(crate) init_channel_panning: [u8; 64],
    pub(crate) init_channel_volume: [u8; 64],
    pub(crate) orders: Vec<Order>,
    pub(crate) instrument_offsets: Vec<u32>,
    pub(crate) sample_offsets: Vec<u32>,
    pub(crate) pattern_offsets: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum Order {
    Index(PatternId),
    Separator,
    EndOfSong,
}

bitflags! {
    pub struct ModuleFlags: u32 {
        // Originally `flags` field. This field has 16 bits however only the lower 8 bits are
        // documented in ITTECH.TXT.

        /// On = Stereo, Off = Mono
        const STEREO = 1 << 0;

        /// If on, no mixing occurs if the volume at mixing time is 0 (redundant v1.04+)
        const VOL_0_MIX_OPTIMIZATIONS = 1 << 1;

        /// On = Use instruments, Off = Use samples
        const USE_INSTRUMENTS = 1 << 2;

        /// On = Linear slides, Off = Amiga slides
        const LINEAR_SLIDES = 1 << 3;

        /// On = Old Effects, Off = IT Effects
        /// Differences:
        /// - Vibrato is updated EVERY frame in IT mode, whereas it is updated every non-row frame
        ///   in other formats.  Also, it is two times deeper with Old Effects ON.
        /// - Command Oxx will set the sample offset to the END of a sample instead of ignoring the
        ///   command under old effects mode.
        /// - (More to come, probably)
        const OLD_EFFECTS = 1 << 4;

        /// On = Link Effect G's memory with Effect E/F. Also Gxx with an instrument present will
        /// cause the envelopes to be retriggered. If you change a sample on a row with Gxx, it'll
        /// adjust the frequency of the current note according to:
        ///
        /// NewFrequency = OldFrequency * NewC5 / OldC5;
        const LINK_G_E_EFFECTS = 1 << 5;

        /// Use MIDI pitch controller, Pitch depth given by PWD
        const USE_MIDI_PITCH = 1 << 6;

        /// Request embedded MIDI configuration
        /// (Coded this way to permit cross-version saving)
        const REQUEST_MIDI_CONFIG_EMBEDDED = 1 << 7;

        // Originally `special` field (shifted by additional 16 bits).

        /// On = song message attached.
        /// Song message:
        ///  Stored at offset given by "Message Offset" field.
        ///  Length = MsgLgth.
        ///  NewLine = 0Dh (13 dec)
        ///  EndOfMsg = 0
        ///
        /// Note: v1.04+ of IT may have song messages of up to
        ///       8000 bytes included.
        #[allow(clippy::identity_op)]
        const MESSAGE_ATTACHED = 1 << (0 + 16);

        /// MIDI configuration embedded
        const MIDI_CONIFG_EMBEDDED = 1 << (3 + 16);
    }
}


impl ModuleFlags {
    pub(crate) fn from_parts(flags: u16, special: u16) -> ModuleFlags {
        let bits = u32::from(flags) | (u32::from(special) << 16);
        ModuleFlags::from_bits_truncate(bits)
    }
}

impl Get<SampleId> for Module {
    type Output = Sample;
    fn get(&self, index: SampleId) -> Option<&Self::Output> {
        self.samples.as_slice().get(usize::from(index.as_u8()))
    }
}

impl_index_from_get!(Module, SampleId);

impl Get<InstrumentId> for Module {
    type Output = Instrument;
    fn get(&self, index: InstrumentId) -> Option<&Self::Output> {
        self.instruments.as_slice().get(usize::from(index.as_u8()))
    }
}

impl_index_from_get!(Module, InstrumentId);

impl Get<PatternId> for Module {
    type Output = Pattern;
    fn get(&self, index: PatternId) -> Option<&Self::Output> {
        self.patterns.as_slice().get(usize::from(index.as_u8()))
    }
}

impl_index_from_get!(Module, PatternId);

impl Module {
    /// Returns an iterator over patterns as listed in the orders list.
    ///
    /// It can yield any pattern multiple times or not yield some patterns at all.
    pub fn ordered_patterns(&self) -> impl Iterator<Item = &Pattern> + '_ {
        self.orders
            .iter()
            .filter_map(move |ord| match ord {
                Order::Index(idx) => self.get(idx),
                _ => None,
            })
    }

    /// Returns active channels when playing the module.
    ///
    /// Does not account for channels in patterns which are not present in the orders list.
    pub fn active_channels(&self) -> ActiveChannels {
        use std::ops::BitOr;

        self.ordered_patterns()
            .map(|pat| pat.active_channels)
            .fold(ActiveChannels::empty(), BitOr::bitor)
    }
}
