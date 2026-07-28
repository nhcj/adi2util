pub mod devices {
    const ADI_2_DAC_FS: u8 = 0x71;

    #[non_exhaustive]
    #[repr(u8)]
    #[derive(Copy, Clone)]
    pub enum Device {
        Adi2DacFs = ADI_2_DAC_FS,
    }

    impl Device {
        pub fn header(&self) -> [u8; 5] {
            [0xF0, 0x00, 0x20, 0x0D, *self as u8]
        }
    }
}

pub mod ipc;
pub mod midi;
pub mod state;
