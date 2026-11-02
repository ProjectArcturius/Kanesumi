// RGBA → BGRA 行转换：按两像素批量换位，让连续源 / 目标行便于编译器生成宽访存。

pub(crate) fn rgba_to_bgra(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(src.len() % 4, 0);
    let mut to = dst.chunks_exact_mut(8);
    let mut from = src.chunks_exact(8);
    for (d, s) in to.by_ref().zip(from.by_ref()) {
        let value = u64::from_le_bytes(s.try_into().unwrap());
        let swapped = (value & 0xff00_ff00_ff00_ff00)
            | ((value & 0x0000_00ff_0000_00ff) << 16)
            | ((value & 0x00ff_0000_00ff_0000) >> 16);
        d.copy_from_slice(&swapped.to_le_bytes());
    }
    for (d, s) in to.into_remainder().chunks_exact_mut(4).zip(from.remainder().chunks_exact(4)) {
        d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::rgba_to_bgra;

    #[test]
    fn channel_swap_preserves_green_alpha_and_odd_pixel_tails() {
        for pixels in 0..=33 {
            let input: Vec<u8> = (0..pixels * 4).map(|i| (i * 31 + i / 7) as u8).collect();
            let mut expected = input.clone();
            for pixel in expected.chunks_exact_mut(4) { pixel.swap(0, 2); }
            let mut output = vec![0; input.len()];
            rgba_to_bgra(&mut output, &input);
            assert_eq!(output, expected, "像素数 {pixels}，包括奇数行尾");
            let mut restored = vec![0; input.len()];
            rgba_to_bgra(&mut restored, &output);
            assert_eq!(restored, input, "两次交换应逐字节还原");
        }
    }
}
