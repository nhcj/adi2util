pub mod devices {
    const ADI_2_DAC_FS: &[u8] = &[0xF0, 0x00, 0x20, 0x0D, 0x71];

    #[non_exhaustive]
    pub enum Device {
        Adi2DacFs,
    }

    impl Device {
        pub fn header(&self) -> &[u8] {
            match self {
                Device::Adi2DacFs => ADI_2_DAC_FS,
            }
        }
    }
}

pub mod ipc;
pub mod midi;
pub mod state;
