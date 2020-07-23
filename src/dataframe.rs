use {
    crate::message::Message,
};

#[inline(always)]
pub fn get_buffer(message: Message) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    buffer.push(129);
    let s = match message {
        Message::Text(s) => s,
        _ => "".to_owned(),
    };
    match s.len() as u64 {
        size @ 0..=125 => {
            buffer.push(size as u8);
        },
        size if size > u32::MAX as u64 => {
            let bytes: [u8; 8] = (size as u64).to_be_bytes();
            buffer.push(127);
            buffer.extend_from_slice(&bytes);
        },
        size if size <= u32::MAX as u64 => {
            let bytes: [u8; 2] = (size as u16).to_be_bytes();
            buffer.push(126);
            buffer.extend_from_slice(&bytes);
        },
        _ => panic!("Don't know what to do here...")
    }
    buffer.extend_from_slice(s.as_bytes());
    buffer
}
#[inline(always)]
/// This is our most performant masking of a &[u8], though it requires exclusive access to the data.
pub fn mask_payload_mut(data: &mut [u8], mask: [u8; 4]) -> &[u8] {
    for i in 0..data.len() {
        let mask_key = mask[(i % 4) as usize];
        data[i] ^= mask_key;
    }
    data
}
#[inline(always)]
pub fn mask_payload(data: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut data = data.to_vec();
    for i in 0..data.len() {
        let mask_key = mask[(i % 4) as usize];
        data[i] ^= mask_key;
    }
    data
}
#[inline(always)]
pub fn mask_payload_vec(mut data: Vec<u8>, mask: [u8; 4]) -> Vec<u8> {
    for i in 0..data.len() {
        let mask_key = mask[(i % 4) as usize];
        data[i] ^= mask_key;
    }
    data
}
pub fn get_message(buffer: &[u8]) -> Message {
    match buffer {
        [_first, second, rest @ ..] => {
            match rest {
                [a, b, c, d, payload @ ..] => {
                    let mut data = payload
                        .into_iter()
                        .take((second & !128) as usize)
                        .map(|&x| x)
                        .collect::<Vec<u8>>();
                    let g = mask_payload(&mut data, [*a, *b, *c, *d]);
                    let text = String::from_utf8_lossy(&g).to_string();
                    Message::Text(text)
                },
                _ => Message::Unknown
            }
        }, // Read first two frames
        _ => Message::Unknown
    }
}

pub mod flat {
    use crate::dataframe::{mask_payload_vec};
    use crate::message::Message;

    mod frame_positions {
        pub const FIN: u8 = 128;
        pub const RSV1: u8 = 64;
        pub const RSV2: u8 = 32;
        pub const RSV3: u8 = 16;

        pub const MASK: u8 = 128;
    }
    #[derive(Debug)]
    pub struct Dataframe {
        data: Vec<u8>
    }
    impl Dataframe {
        pub fn new(data: Vec<u8>) -> Self {
            Self { data }
        }
        pub fn get_fin(&self) -> bool {
            self.data
                .get(0)
                .map(|&s| ((frame_positions::FIN & s) == frame_positions::FIN))
                .unwrap_or(false)
        }
        pub fn get_rsv1(&self) -> bool {
            self.data
                .get(0)
                .map(|&s| ((frame_positions::RSV1 & s) == frame_positions::RSV1))
                .unwrap_or(false)
        }
        pub fn get_rsv2(&self) -> bool {
            self.data
                .get(0)
                .map(|&s| ((frame_positions::RSV2 & s) == frame_positions::RSV2))
                .unwrap_or(false)
        }
        pub fn get_rsv3(&self) -> bool {
            self.data
                .get(0)
                .map(|&s| ((frame_positions::RSV3 & s) == frame_positions::RSV3))
                .unwrap_or(false)
        }
        pub fn get_opcode(&self) -> u8 {
            self.data
                .get(0)
                // Get far most right bits
                .map(|&s| s & 0b00001111)
                .unwrap_or(0)
        }
        pub fn get_mask(&self) -> bool {
            self.data
                .get(1)
                .map(|&s| ((frame_positions::MASK & s) == frame_positions::MASK))
                .unwrap_or(false)
        }
        pub fn get_payload_length_from_second_frame(&self) -> u8 {
            self.data
                .get(1)
                .map(|s| s & 0b01111111)
                .unwrap_or(0)
        }
        pub fn get_payload_length(&self) -> u64 {
            match self.get_payload_length_from_second_frame() {
                size @ 0..=125 => size as u64,
                126 => u32::from_be_bytes([0, 0, self.data[2], self.data[3]]) as u64,
                127 => u64::from_be_bytes([self.data[2], self.data[3], self.data[4], self.data[5], self.data[6], self.data[7], self.data[8], self.data[9]]),
                size => panic!("Payload length not allowed: {}", size),
            }
        }
        pub fn get_masking_key(&self) -> [u8; 4] {
            let mut masking_key: [u8; 4] = [0; 4];
            let data = self.data.iter();
            let buffer = match self.get_payload_length_from_second_frame() {
                0..=125 => data.skip(2),
                126 => data.skip(4),
                127 => data.skip(10),
                size => panic!("Masking key not allowed: {}", size),
            }.take(4).map(|&s| s).collect::<Vec<u8>>();
            masking_key.copy_from_slice(&buffer);
            masking_key
        }
        pub fn get_payload(&self) -> Vec<u8> {
            let data = self.data.clone().into_iter();
            match self.get_payload_length_from_second_frame() {
                0..=125 => data.skip(6),
                126 => data.skip(8),
                127 => data.skip(14),
                size => panic!("Payload not allowed: {}", size),
            }.take(self.get_payload_length() as usize).collect()
        }
        pub fn get_extended_payload_length(&self) -> u64 {
            self.get_payload_length_from_second_frame() as u64
        }
        pub fn get_masking_key_size(&self) -> u8 {
            4
        }
        pub fn get_initial_frame_size(&self) -> u8 {
            2
        }
        pub fn get_extra_payload_size(&self) -> u8 {
            match self.get_payload_length_from_second_frame() {
                127 => 8,
                126 => 2,
                0..=125 => 0,
                _ => panic!("Invalid size"),
            }
        }
        pub fn get_full_payload_size(&self) -> u64 {
            self.get_initial_frame_size() as u64 + self.get_masking_key_size() as u64 + self.get_extra_payload_size() as u64 + self.get_payload_length()
        }
        pub fn get_unmasked_payload(&self) -> Vec<u8> {
            match self.get_mask() {
                true => mask_payload_vec(self.get_payload(), self.get_masking_key()),
                false => self.get_payload().to_vec(),
            }
        }
        pub fn get_message(&self) -> Option<Message> {
            match self.get_opcode() {
                1 => Some(Message::Text(String::from_utf8_lossy(&self.get_unmasked_payload()).to_string())),
                2 => Some(Message::Binary(self.get_unmasked_payload())),
                _ => None
            }
        }
    }
    impl From<Vec<u8>> for Dataframe {
        fn from(vec: Vec<u8>) -> Self {
            Dataframe::new(vec)
        }
    }
    impl From<&Vec<u8>> for Dataframe {
        fn from(vec: &Vec<u8>) -> Self {
            Dataframe::new(vec.to_owned())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::message::Message;

        #[test]
        fn test_buffer_hello_world() {
            let str = "Hello World";
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let dataframe = Dataframe::new(buffer);
            assert!(dataframe.get_fin());
            assert!(dataframe.get_mask());
            assert_eq!(dataframe.get_payload_length_from_second_frame(), str.len() as u8);
            assert_eq!(dataframe.get_payload_length(), str.len() as u64);
            assert_eq!(String::from_utf8(dataframe.get_unmasked_payload().to_vec()).unwrap().as_str(), str);
        }
        #[test]
        fn test_payload_size() {
            let s = (0..488376).map(|_| "a").collect::<String>();
            let v = vec![129, 255, 0, 0, 0, 0, 0, 7, 115, 184, 105, 143, 80, 179];
            let dataframe = Dataframe::new(v);
            assert_eq!(dataframe.get_payload_length(), s.len() as u64);
        }

        #[test]
        fn test_buffer_to_dataframe() {
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let dataframe: Dataframe = buffer.into();
            dbg!(dataframe);
        }
        #[test]
        fn test_buffer_126_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
            let buffer: Vec<u8> = vec![129,254,0,126,202,250,57,41,178,160,113,93,136,159,113,75,186,185,110,106,158,185,86,83,132,141,9,110,178,187,93,120,242,171,72,88,190,159,65,28,144,144,92,17,140,184,88,127,155,138,65,91,163,157,65,16,248,184,73,101,147,163,80,113,144,148,120,104,253,202,122,77,132,137,85,126,188,157,93,122,135,128,9,95,172,175,94,78,140,194,108,17,189,136,108,101,144,128,14,71,185,203,77,124,163,207,123,109,157,151,65,81,250,162,106,28,134,137,123,76,179,188,76,72,137,139,13,103,142,187,79,94,168,147];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_126_overflow_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
            let buffer: Vec<u8> = vec![129,254,0,126,202,250,57,41,178,160,113,93,136,159,113,75,186,185,110,106,158,185,86,83,132,141,9,110,178,187,93,120,242,171,72,88,190,159,65,28,144,144,92,17,140,184,88,127,155,138,65,91,163,157,65,16,248,184,73,101,147,163,80,113,144,148,120,104,253,202,122,77,132,137,85,126,188,157,93,122,135,128,9,95,172,175,94,78,140,194,108,17,189,136,108,101,144,128,14,71,185,203,77,124,163,207,123,109,157,151,65,81,250,162,106,28,134,137,123,76,179,188,76,72,137,139,13,103,142,187,79,94,168,147,0,0,0,0];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_127_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
            let buffer: Vec<u8> = vec![129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170, 114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154, 140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11, 220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154, 73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153, 155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93, 74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168, 83, 69, 140, 128, 68];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_127_overflow_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
            let buffer: Vec<u8> = vec![129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170, 114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154, 140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11, 220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154, 73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153, 155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93, 74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168, 83, 69, 140, 128, 68, 0, 0, 0, 0];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_large() {
            let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
            let buffer: Vec<u8> = vec![129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_overflow_large() {
            let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
            let buffer: Vec<u8> = vec![129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164,0,0,0,0];
            let dataframe: Dataframe = buffer.into();
            let message = dataframe.get_message().unwrap();
            assert_eq!(message, Message::Text(str.to_string()));
        }
    }
}
pub mod structered {
    use std::ops::Deref;
    use crate::message::Message;
    use crate::dataframe::mask_payload;

    #[derive(Debug)]
    enum Fin {
        True,
        False
    }
    #[derive(Debug)]
    enum Rsv1 {
        True,
        False
    }
    #[derive(Debug)]
    enum Rsv2 {
        True,
        False
    }
    #[derive(Debug)]
    enum Rsv3 {
        True,
        False
    }
    #[derive(Debug)]
    enum Mask {
        True,
        False
    }
    #[derive(Debug)]
    struct Opcode(u8);
    #[derive(Debug)]
    struct PayloadLength(u64);
    #[derive(Debug)]
    struct MaskingKey([u8; 4]);
    #[derive(Debug)]
    pub struct DataframeHead {
        fin: Fin,
        rsv1: Rsv1,
        rsv2: Rsv2,
        rsv3: Rsv3,
        opcode: Opcode,
    }
    #[derive(Debug)]
    pub struct DataframeBody {
        mask: Mask,
        payload_length: PayloadLength,
        extended_payload_length: PayloadLength,
        masking_key: MaskingKey,
    }
    #[derive(Debug)]
    pub struct DataframeData(Vec<u8>);
    #[derive(Debug)]
    pub struct Dataframe {
        head: Option<DataframeHead>,
        body: Option<DataframeBody>,
        data: Option<DataframeData>,
    }

    impl From<u8> for Fin {
        fn from(b: u8) -> Self {
            match (128 & b) == 128 {
                true => Fin::True,
                false => Fin::False
            }
        }
    }
    impl From<u8> for Mask {
        fn from(b: u8) -> Self {
            match (128 & b) == 128 {
                true => Mask::True,
                false => Mask::False
            }
        }
    }
    impl From<u8> for Rsv1 {
        fn from(b: u8) -> Self {
            match (64 & b) == 64 {
                true => Rsv1::True,
                false => Rsv1::False,
            }
        }
    }
    impl From<u8> for Rsv2 {
        fn from(b: u8) -> Self {
            match (32 & b) == 32 {
                true => Rsv2::True,
                false => Rsv2::False,
            }
        }
    }
    impl From<u8> for Rsv3 {
        fn from(b: u8) -> Self {
            match (16 & b) == 16 {
                true => Rsv3::True,
                false => Rsv3::False,
            }
        }
    }
    impl Opcode {
        pub fn get_opcode(&self) -> u8 {
            self.0
        }
    }
    impl From<u8> for Opcode {
        fn from(b: u8) -> Self {
            // Remove Fin, Rsv1, Rsv2 and Rsv3 to get out opcode
            Opcode(b & 0b00001111)
        }
    }
    impl PayloadLength {
        pub fn new(size: u64) -> Self {
            Self(size)
        }
        pub fn get_payload_length(&self) -> u64 {
            self.0
        }
    }
    impl From<u64> for PayloadLength {
        fn from(size: u64) -> Self {
            PayloadLength::new(size)
        }
    }
    impl From<u8> for PayloadLength {
        fn from(size: u8) -> Self {
            PayloadLength::new((size & 0b01111111) as u64)
        }
    }
    impl MaskingKey {
        pub fn get_masking_key(&self) -> [u8; 4] {
            self.0
        }
    }
    impl Deref for Opcode {
        type Target = u8;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl Deref for PayloadLength {
        type Target = u64;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl DataframeHead {
        pub fn new(byte: u8) -> Self {
            let fin = byte.into();
            let rsv1 = byte.into();
            let rsv2 = byte.into();
            let rsv3 = byte.into();
            let opcode = byte.into();
            Self {
                fin,
                rsv1,
                rsv2,
                rsv3,
                opcode
            }
        }
    }
    impl DataframeData {
        pub fn new(data: Vec<u8>) -> Self {
            Self(data)
        }
        pub fn get_payload(&self) -> &Vec<u8> {
            &self.0
        }
    }
    impl DataframeBody {
        pub fn new(byte: u8, rest: &[u8]) -> Self {
            let mask = byte.into();
            let payload_length: PayloadLength = byte.into();

            let (extended_payload_length, masking_key) = match payload_length.get_payload_length() as u64 {
                127 if rest.len() > 11 => {
                    let size = u64::from_be_bytes([rest[0], rest[1], rest[2], rest[3], rest[4], rest[5], rest[6], rest[7]]);
                    let masking_key = MaskingKey([rest[8], rest[9], rest[10], rest[11]]);
                    (PayloadLength::new(size), masking_key)
                },
                126 if rest.len() > 5 => {
                    let size = (rest[0] | rest[1]) as u64;
                    let masking_key = MaskingKey([rest[2], rest[3], rest[4], rest[5]]);
                    (PayloadLength::new(size), masking_key)
                },
                size@ 0..=125 if rest.len() > 3 => {
                    // TODO: Fetch from rest
                    let masking_key = MaskingKey([rest[0], rest[1], rest[2], rest[3]]);
                    (PayloadLength::new(size), masking_key)
                },
                _ => panic!("Invalid size"),
            };
            Self {
                mask,
                payload_length,
                extended_payload_length,
                masking_key,
            }
        }
        pub fn get_extra_payload_size(&self) -> u8 {
            match self.payload_length.get_payload_length() {
                127 => 8,
                126 => 2,
                0..=125 => 0,
                _ => panic!("Invalid size"),
            }
        }
        pub fn get_extended_payload_length(&self) -> u64 {
            self.extended_payload_length.get_payload_length()
        }
        pub fn get_masking_key_size(&self) -> u8 {
            4
        }
        pub fn get_initial_frame_size(&self) -> u8 {
            2
        }
        pub fn get_full_payload_size(&self) -> u64 {
            self.get_initial_frame_size() as u64 + self.get_masking_key_size() as u64 + self.get_extra_payload_size() as u64 + self.get_extended_payload_length()
        }
    }
    impl Dataframe {
        pub fn new(head: Option<DataframeHead>, body: Option<DataframeBody>, data: Option<DataframeData>) -> Self {
            Self { head, body, data }
        }
        pub fn get_message(&self) -> Option<Message> {
            let opcode = self.head.as_ref().map(|head| head.opcode.get_opcode());
            let payload = self.data.as_ref().map(|data| data.get_payload());

            match (opcode, payload) {
                (Some(opcode), Some(payload)) => match opcode {
                    1 => {
                        // TODO: Remove unwrap here
                        let mask = self.body.as_ref().map(|body| body.masking_key.get_masking_key()).unwrap();
                        let masked_data = mask_payload(payload, mask);
                        let message = Message::Text(String::from_utf8_lossy(&masked_data).to_string());
                        Some(message)
                    },
                    _ => None
                },
                _ => None
            }
        }
    }
    impl From<&[u8]> for Dataframe {
        fn from(frames: &[u8]) -> Self {
            match frames {
                [first, second, rest @ ..] => {
                    let dataframe_head = DataframeHead::new(*first);
                    let dataframe_body = DataframeBody::new(*second, rest);
                    let start_pos = 4 + dataframe_body.get_extra_payload_size() as usize;
                    let end_pos = start_pos + dataframe_body.get_extended_payload_length() as usize;
                    let dataframe_data = DataframeData::new(rest[start_pos..end_pos].to_vec());
                    Dataframe::new(Some(dataframe_head), Some(dataframe_body), Some(dataframe_data))
                },
                _ => Dataframe::new(None, None, None)
            }
        }
    }
    impl From<&Vec<u8>> for Dataframe {
        fn from(frames: &Vec<u8>) -> Self {
            frames.as_slice().into()
        }
    }
    impl From<Vec<u8>> for Dataframe {
        fn from(frames: Vec<u8>) -> Self {
            frames.as_slice().into()
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::message::Message;
        use crate::dataframe::get_message;

        #[test]
        fn test_buffer_hello_world() {
            let str = "Hello World";
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let message = get_message(&buffer);
            assert_eq!(message, Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_to_dataframe() {
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let dataframe: Dataframe = buffer.as_slice().into();
            dbg!(dataframe);
        }
        #[test]
        fn test_buffer_126_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
            let buffer: Vec<u8> = vec![129,254,0,126,202,250,57,41,178,160,113,93,136,159,113,75,186,185,110,106,158,185,86,83,132,141,9,110,178,187,93,120,242,171,72,88,190,159,65,28,144,144,92,17,140,184,88,127,155,138,65,91,163,157,65,16,248,184,73,101,147,163,80,113,144,148,120,104,253,202,122,77,132,137,85,126,188,157,93,122,135,128,9,95,172,175,94,78,140,194,108,17,189,136,108,101,144,128,14,71,185,203,77,124,163,207,123,109,157,151,65,81,250,162,106,28,134,137,123,76,179,188,76,72,137,139,13,103,142,187,79,94,168,147];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_126_overflow_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
            let buffer: Vec<u8> = vec![129,254,0,126,202,250,57,41,178,160,113,93,136,159,113,75,186,185,110,106,158,185,86,83,132,141,9,110,178,187,93,120,242,171,72,88,190,159,65,28,144,144,92,17,140,184,88,127,155,138,65,91,163,157,65,16,248,184,73,101,147,163,80,113,144,148,120,104,253,202,122,77,132,137,85,126,188,157,93,122,135,128,9,95,172,175,94,78,140,194,108,17,189,136,108,101,144,128,14,71,185,203,77,124,163,207,123,109,157,151,65,81,250,162,106,28,134,137,123,76,179,188,76,72,137,139,13,103,142,187,79,94,168,147,0,0,0,0];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_127_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
            let buffer: Vec<u8> = vec![129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170, 114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154, 140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11, 220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154, 73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153, 155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93, 74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168, 83, 69, 140, 128, 68];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_127_overflow_length() {
            let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
            let buffer: Vec<u8> = vec![129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170, 114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154, 140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11, 220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154, 73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153, 155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93, 74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168, 83, 69, 140, 128, 68, 0, 0, 0, 0];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_large() {
            let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
            let buffer: Vec<u8> = vec![129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
        #[test]
        fn test_buffer_overflow_large() {
            let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
            let buffer: Vec<u8> = vec![129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164,0,0,0,0];
            let dataframe: Dataframe = buffer.as_slice().into();
            assert_eq!(dataframe.get_message().unwrap(), Message::Text(str.to_string()));
        }
    }
}