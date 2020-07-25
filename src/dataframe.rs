use {
    crate::{
        WsGonzaleResult,
        WsGonzaleError,
        message::Message
    },
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
        }
        size if size > u32::MAX as u64 => {
            let bytes: [u8; 8] = (size as u64).to_be_bytes();
            buffer.push(127);
            buffer.extend_from_slice(&bytes);
        }
        size if size <= u32::MAX as u64 => {
            let bytes: [u8; 2] = (size as u16).to_be_bytes();
            buffer.push(126);
            buffer.extend_from_slice(&bytes);
        }
        _ => panic!("Don't know what to do here..."),
    }
    buffer.extend_from_slice(s.as_bytes());
    buffer
}
#[inline(always)]
pub fn super_mask<'a, 'b>(incoming: &'a mut &'b mut [u8], mask: [u8; 4]) -> &'a [u8] {
    let data: &'b mut [u8] = std::mem::take(incoming);
    for i in 0..data.len() {
        data[i] ^= mask[i % 4];
    }
    data
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

pub struct DataframeBuilder(Vec<u8>);
#[derive(Debug)]
pub struct Dataframe {
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    is_mask: bool,
    opcode: u8,
    payload_length: u64,
    full_frame_length: u64,
    masking_key: [u8; 4],
    payload: Vec<u8>,
}

pub enum Opcode {
    Continuation = 0,
    Text = 1,
    Close = 8,
    Ping = 9,
    Pong = 10,
    Unknown,
}

#[derive(Debug)]
enum ExtraSize {
    Zero(u8),
    Two,
    Eight,
}
mod frame_positions {
    // Frame one
    pub const FIN: u8 = 128;
    pub const RSV1: u8 = 64;
    pub const RSV2: u8 = 32;
    pub const RSV3: u8 = 16;
    pub const MASK_OPCODE: u8 = 0b00001111;
    // Frame two
    pub const IS_MASK: u8 = 128;
    pub const MASK_PAYLOAD_LENGTH: u8 = 0b01111111;
}
impl DataframeBuilder {
    pub fn new(buffer: Vec<u8>) -> WsGonzaleResult<Dataframe> {
        DataframeBuilder(buffer).get_dataframe()
    }
    #[inline(always)]
    fn is_fin(&self) -> bool {
        self.0
            .get(0)
            .map(|frame| (frame & frame_positions::FIN) == frame_positions::FIN)
            .unwrap_or(false)
    }
    #[inline(always)]
    fn is_rsv1(&self) -> bool {
        self.0
            .get(0)
            .map(|frame| (frame & frame_positions::RSV1) == frame_positions::RSV1)
            .unwrap_or(false)
    }
    #[inline(always)]
    fn is_rsv2(&self) -> bool {
        self.0
            .get(0)
            .map(|frame| (frame & frame_positions::RSV2) == frame_positions::RSV2)
            .unwrap_or(false)
    }
    #[inline(always)]
    fn is_rsv3(&self) -> bool {
        self.0
            .get(0)
            .map(|frame| (frame & frame_positions::RSV3) == frame_positions::RSV3)
            .unwrap_or(false)
    }
    #[inline(always)]
    /// Get the last four bits in one byte in first frame
    fn get_opcode(&self) -> u8 {
        // default to close
        self.0
            .get(0)
            .map(|frame| frame & frame_positions::MASK_OPCODE)
            .unwrap_or(8)
    }
    #[inline(always)]
    fn is_mask(&self) -> bool {
        self.0
            .get(1)
            .map(|frame| (frame & frame_positions::IS_MASK) == frame_positions::IS_MASK)
            .unwrap_or(false)
    }
    /// Get the last seven bits in the byte in the second frame
    #[inline(always)]
    fn get_short_payload_length(&self) -> u8 {
        self.0
            .get(1)
            .map(|frame| frame & frame_positions::MASK_PAYLOAD_LENGTH)
            .unwrap_or(0)
    }
    #[inline(always)]
    fn get_extra_payload_bytes(&self) -> WsGonzaleResult<ExtraSize> {
        let result = match self.get_short_payload_length() {
            size @ 0..=125 => ExtraSize::Zero(size),
            126 => ExtraSize::Two,
            127 => ExtraSize::Eight,
            _ => unreachable!("Max payload for a dataframe in WS spec is 127"),
        };
        Ok(result)
    }
    #[inline(always)]
    fn get_payload_length(&self) -> WsGonzaleResult<u64> {
        let slice = self.0.as_slice();
        let result = match self.get_extra_payload_bytes()? {
            ExtraSize::Zero(size) => size as u64,
            ExtraSize::Two => match slice {
                [_, _, first, second, ..] if slice.len() > 4 => {
                    u32::from_be_bytes([0, 0, *first, *second]) as u64
                }
                _ => return Err(WsGonzaleError::Unknown),
            },
            ExtraSize::Eight => match slice {
                [_, _, first, second, third, fourth, fifth, sixth, seventh, eighth, ..]
                    if slice.len() > 8 =>
                {
                    u64::from_be_bytes([
                        *first, *second, *third, *fourth, *fifth, *sixth, *seventh, *eighth,
                    ]) as u64
                }
                _ => return Err(WsGonzaleError::Unknown),
            },
        };

        Ok(result)
    }

    fn get_payload_start_pos(&self) -> WsGonzaleResult<u64> {
        let result = match self.get_extra_payload_bytes()? {
            ExtraSize::Zero(_) => 6,
            ExtraSize::Two => 8,
            ExtraSize::Eight => 14,
        };
        Ok(result)
    }
    pub fn get_full_frame_length(&self) -> WsGonzaleResult<u64> {
        let size = self.get_payload_start_pos()? + self.get_payload_length()?;

        Ok(size)
    }
    #[inline(always)]
    fn get_masking_key_start(&self) -> WsGonzaleResult<u8> {
        let result = match self.get_extra_payload_bytes()? {
            ExtraSize::Zero(_) => 0,
            ExtraSize::Two => 2,
            ExtraSize::Eight => 8,
        };
        Ok(result)
    }
    #[inline(always)]
    fn get_masking_key(&self) -> WsGonzaleResult<[u8; 4]> {
        let start = 2 + self.get_masking_key_start()? as usize;
        let end = start + 4;
        if self.is_mask() && self.0.len() >= end {
            let mut buffer: [u8; 4] = [0; 4];
            buffer.copy_from_slice(&self.0[start..end]);
            Ok(buffer)
        } else {
            // masking key [0, 0, 0, 0] is ok because 1 ^ 0 == 1, 0 ^ 0 == 0
            Ok([0, 0, 0, 0])
        }
    }
    #[inline(always)]
    fn get_payload(mut self) -> WsGonzaleResult<Vec<u8>> {
        let start_payload = self.get_payload_start_pos()? as usize;
        let is_mask = self.is_mask();
        let masking_key = self.get_masking_key()?;
        let payload_length = self.get_payload_length()? as usize;

        // TODO: Make use of Opcode enum
        if self.get_opcode() == 8 {
            // TODO: Add support for reason message for closing
            return Err(WsGonzaleError::ConnectionClosed);
        }
        if start_payload > self.0.len() {
            return Err(WsGonzaleError::InvalidPayload);
        }
        // Remove first {start_payload}:th bytes from dataframe payload
        self.0.drain(0..start_payload);
        let mut data = self.0.into_iter().take(payload_length).collect::<Vec<u8>>();
        if is_mask {
            super_mask(&mut &mut *data, masking_key);
        }
        Ok(data)
    }
    #[inline(always)]
    fn get_dataframe(self) -> WsGonzaleResult<Dataframe> {
        let result = Dataframe {
            fin: self.is_fin(),
            rsv1: self.is_rsv1(),
            rsv2: self.is_rsv2(),
            rsv3: self.is_rsv3(),
            is_mask: self.is_mask(),
            opcode: self.get_opcode(),
            payload_length: self.get_payload_length()?,
            full_frame_length: self.get_full_frame_length()?,
            masking_key: self.get_masking_key()?,
            payload: self.get_payload()?,
        };
        Ok(result)
    }
}
impl Dataframe {
    #[inline(always)]
    pub fn get_message(self) -> WsGonzaleResult<Message> {
        let result = match self.opcode {
            1 => Message::Text(
                String::from_utf8_lossy(&self.get_payload())
                    .parse()
                    .map_err(|_| WsGonzaleError::InvalidPayload)?,
            ),
            8 => Message::Close,
            _ => Message::Unknown,
        };
        Ok(result)
    }
    #[inline(always)]
    pub fn is_fin(&self) -> bool {
        self.fin
    }
    #[inline(always)]
    pub fn is_rsv1(&self) -> bool {
        self.rsv1
    }
    #[inline(always)]
    pub fn is_rsv2(&self) -> bool {
        self.rsv2
    }
    #[inline(always)]
    pub fn is_rsv3(&self) -> bool {
        self.rsv3
    }
    #[inline(always)]
    pub fn get_opcode(&self) -> u8 {
        self.opcode
    }
    #[inline(always)]
    pub fn is_mask(&self) -> bool {
        self.is_mask
    }
    #[inline(always)]
    pub fn get_payload_length(&self) -> u64 {
        self.payload_length
    }
    pub fn get_full_frame_length(&self) -> u64 {
        self.full_frame_length
    }
    #[inline(always)]
    pub fn get_payload(self) -> Vec<u8> {
        self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;
    #[test]
    #[should_panic]
    fn test_buffer_with_no_payload_or_masking_key_but_payload_length() {
        let buffer: Vec<u8> = vec![
            129, // FIN(128) + Opcode(1)
            129, // MASK(128) + PayloadLength(1)
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        dataframe.get_message().unwrap();
    }
    #[test]
    fn test_buffer_with_no_payload_but_masking_key_and_payload_length() {
        let buffer: Vec<u8> = vec![
            129, // FIN(128) + Opcode(1)
            129, // MASK(128) + PayloadLength(1)
            0, 0, 0, 0,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        dataframe.get_message().unwrap();
    }
    #[test]
    fn test_buffer_with_no_payload_or_mask() {
        let buffer: Vec<u8> = vec![
            129, // FIN(128) + Opcode(1)
            0,
        ];
        let result = DataframeBuilder::new(buffer);
        assert_eq!(result.err().unwrap(), WsGonzaleError::InvalidPayload);
    }
    #[test]
    fn test_close_frame_from_client() {
        let buffer: Vec<u8> = vec![
            136, // FIN(128) + Opcode(8)
            128, // MASK(128)
        ];
        let result = DataframeBuilder::new(buffer);
        assert_eq!(result.err().unwrap(), WsGonzaleError::ConnectionClosed);
    }
    #[test]
    fn test_buffer_with_no_payload_with_masking_key() {
        let buffer: Vec<u8> = vec![
            129, // FIN(128) + Opcode(1)
            128, // MASK(128)
            0, 0, 0, 0,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        dataframe.get_message().unwrap();
    }
    #[test]
    fn test_buffer_hello_world() {
        let str = "Hello World";
        let buffer: Vec<u8> = vec![
            129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        dbg!(&dataframe);
        assert!(dataframe.is_fin());
        assert!(dataframe.is_mask());
        assert_eq!(
            String::from_utf8(dataframe.get_payload().to_vec())
                .unwrap()
                .as_str(),
            str
        );
    }
    #[test]
    fn test_payload_size() {
        let s = (0..488376).map(|_| "a").collect::<String>();
        let buffer = vec![129, 255, 0, 0, 0, 0, 0, 7, 115, 184, 105, 143, 80, 179];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        assert_eq!(dataframe.get_payload_length(), s.len() as u64);
    }

    #[test]
    fn test_buffer_to_dataframe() {
        let buffer: Vec<u8> = vec![
            129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        dbg!(dataframe);
    }
    #[test]
    fn test_buffer_126_length() {
        let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 126, 202, 250, 57, 41, 178, 160, 113, 93, 136, 159, 113, 75, 186, 185,
            110, 106, 158, 185, 86, 83, 132, 141, 9, 110, 178, 187, 93, 120, 242, 171, 72, 88, 190,
            159, 65, 28, 144, 144, 92, 17, 140, 184, 88, 127, 155, 138, 65, 91, 163, 157, 65, 16,
            248, 184, 73, 101, 147, 163, 80, 113, 144, 148, 120, 104, 253, 202, 122, 77, 132, 137,
            85, 126, 188, 157, 93, 122, 135, 128, 9, 95, 172, 175, 94, 78, 140, 194, 108, 17, 189,
            136, 108, 101, 144, 128, 14, 71, 185, 203, 77, 124, 163, 207, 123, 109, 157, 151, 65,
            81, 250, 162, 106, 28, 134, 137, 123, 76, 179, 188, 76, 72, 137, 139, 13, 103, 142,
            187, 79, 94, 168, 147,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
    #[test]
    fn test_buffer_126_overflow_length() {
        let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbi";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 126, 202, 250, 57, 41, 178, 160, 113, 93, 136, 159, 113, 75, 186, 185,
            110, 106, 158, 185, 86, 83, 132, 141, 9, 110, 178, 187, 93, 120, 242, 171, 72, 88, 190,
            159, 65, 28, 144, 144, 92, 17, 140, 184, 88, 127, 155, 138, 65, 91, 163, 157, 65, 16,
            248, 184, 73, 101, 147, 163, 80, 113, 144, 148, 120, 104, 253, 202, 122, 77, 132, 137,
            85, 126, 188, 157, 93, 122, 135, 128, 9, 95, 172, 175, 94, 78, 140, 194, 108, 17, 189,
            136, 108, 101, 144, 128, 14, 71, 185, 203, 77, 124, 163, 207, 123, 109, 157, 151, 65,
            81, 250, 162, 106, 28, 134, 137, 123, 76, 179, 188, 76, 72, 137, 139, 13, 103, 142,
            187, 79, 94, 168, 147, 0, 0, 0, 0,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
    #[test]
    fn test_buffer_127_length() {
        let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170,
            114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154,
            140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11,
            220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154,
            73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153,
            155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93,
            74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168,
            83, 69, 140, 128, 68,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
    #[test]
    fn test_buffer_127_overflow_length() {
        let str = "xZHtBeHbpCWCTCozNw0GxAdQ8Qqqtex5Zje8FBaVQpxrigx92BpLYYiXZnAA70CdNslWvgdSMz0vfUggF8U8wrULZz7ns1tUi5BDWmxx0XS5LsBeyFuaCq4NDAvwbia";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 127, 238, 233, 37, 50, 150, 179, 109, 70, 172, 140, 109, 80, 158, 170,
            114, 113, 186, 170, 74, 72, 160, 158, 21, 117, 150, 168, 65, 99, 214, 184, 84, 67, 154,
            140, 93, 7, 180, 131, 64, 10, 168, 171, 68, 100, 191, 153, 93, 64, 135, 142, 93, 11,
            220, 171, 85, 126, 183, 176, 76, 106, 180, 135, 100, 115, 217, 217, 102, 86, 160, 154,
            73, 101, 152, 142, 65, 97, 163, 147, 21, 68, 136, 188, 66, 85, 168, 209, 112, 10, 153,
            155, 112, 126, 180, 147, 18, 92, 157, 216, 81, 103, 135, 220, 103, 118, 185, 132, 93,
            74, 222, 177, 118, 7, 162, 154, 103, 87, 151, 175, 80, 83, 173, 152, 17, 124, 170, 168,
            83, 69, 140, 128, 68, 0, 0, 0, 0,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
    #[test]
    fn test_buffer_large() {
        let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119,
            246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246,
            164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164,
            253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239,
            119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101,
            225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225,
            161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164,
            253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239,
            119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119,
            225, 179, 253, 114, 228, 164,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
    #[test]
    fn test_buffer_overflow_large() {
        let str = "asdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadadasdsadasdasdadsadad";
        let buffer: Vec<u8> = vec![
            129, 254, 0, 152, 156, 22, 133, 192, 253, 101, 225, 179, 253, 114, 228, 179, 248, 119,
            246, 164, 253, 114, 246, 161, 248, 119, 225, 161, 239, 114, 246, 161, 248, 119, 246,
            164, 253, 101, 225, 161, 248, 101, 228, 164, 253, 114, 228, 179, 248, 101, 228, 164,
            253, 101, 225, 161, 239, 114, 228, 164, 239, 119, 225, 161, 248, 119, 246, 164, 239,
            119, 225, 161, 239, 114, 228, 179, 248, 119, 225, 179, 253, 114, 228, 164, 253, 101,
            225, 179, 253, 114, 228, 179, 248, 119, 246, 164, 253, 114, 246, 161, 248, 119, 225,
            161, 239, 114, 246, 161, 248, 119, 246, 164, 253, 101, 225, 161, 248, 101, 228, 164,
            253, 114, 228, 179, 248, 101, 228, 164, 253, 101, 225, 161, 239, 114, 228, 164, 239,
            119, 225, 161, 248, 119, 246, 164, 239, 119, 225, 161, 239, 114, 228, 179, 248, 119,
            225, 179, 253, 114, 228, 164, 0, 0, 0, 0,
        ];
        let dataframe: Dataframe = DataframeBuilder::new(buffer).unwrap();
        let message = dataframe.get_message().unwrap();
        assert_eq!(message, Message::Text(str.to_string()));
    }
}
