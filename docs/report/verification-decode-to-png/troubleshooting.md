# Troubleshooting: openh264 decode error (AVCC format)

## エラー内容
```
Error: decode error: openh264 decode error: OpenH264 encountered an error. Native:4. Decoding State:0.
```

## 原因
MP4 コンテナ内の H.264 サンプルデータは **AVCC (length-prefixed)** 形式で格納されている:
```
[4 bytes: NAL length (big-endian)] [NAL data] [4 bytes: NAL length] [NAL data] ...
```

openh264 は **Annex B (start-code prefixed)** 形式を期待する:
```
[00 00 00 01] [NAL data] [00 00 00 01] [NAL data] ...
```

さらに、H.264 デコーダは最初にSPS (Sequence Parameter Set) と PPS (Picture Parameter Set) を受け取る必要がある。
これらは MP4 では `avcC` ボックスの `extra_data` に格納されており、サンプルストリームには含まれない。

## 解決策
`demux/mp4.rs` に 2 つのヘルパー関数を追加:

### 1. `avcc_to_annex_b(avcc_data, nal_length_size, out)`
- AVCC length prefix を Annex B start code (00 00 00 01) に置換
- nal_length_size (通常 4) バイトの big-endian 長をパース

### 2. `annex_b_from_avcc_extra(extra, out)`
- avcC ボックスのバイナリ構造をパースして SPS/PPS NAL units を抽出
- 各 NAL に Annex B start code を付与

### 3. `next_packet()` での適用
- キーフレーム時: まず `annex_b_from_avcc_extra()` で SPS/PPS を出力
- 全サンプル: `avcc_to_annex_b()` で NAL データを変換

## avcC ボックス構造 (参考)
```
offset  field
[0]     configurationVersion (1)
[1]     AVCProfileIndication
[2]     profile_compatibility
[3]     AVCLevelIndication
[4]     lengthSizeMinusOne (lower 2 bits) → nal_length_size = (val & 0x03) + 1
[5]     numSPS (lower 5 bits)
        for each SPS: [2 bytes length] [SPS NAL bytes]
        numPPS (1 byte)
        for each PPS: [2 bytes length] [PPS NAL bytes]
```
