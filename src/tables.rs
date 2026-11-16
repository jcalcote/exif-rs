// Keep literal source table rows auditable against the pinned Java maps.
#![allow(clippy::too_many_lines, clippy::match_same_arms)]
// Adapted from metadata-extractor 2.19.0, Copyright 2002-2019 Drew Noakes and contributors.
// Licensed under Apache-2.0; see resources/LICENSE-APACHE-2.0 and provenance.md.

pub(crate) fn exif_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x0001 => Some("Interoperability Index"),
        0x0002 => Some("Interoperability Version"),
        0x00fe => Some("New Subfile Type"),
        0x00ff => Some("Subfile Type"),
        0x0100 => Some("Image Width"),
        0x0101 => Some("Image Height"),
        0x0102 => Some("Bits Per Sample"),
        0x0103 => Some("Compression"),
        0x0106 => Some("Photometric Interpretation"),
        0x0107 => Some("Thresholding"),
        0x010a => Some("Fill Order"),
        0x010d => Some("Document Name"),
        0x010e => Some("Image Description"),
        0x010f => Some("Make"),
        0x0110 => Some("Model"),
        0x0111 => Some("Strip Offsets"),
        0x0112 => Some("Orientation"),
        0x0115 => Some("Samples Per Pixel"),
        0x0116 => Some("Rows Per Strip"),
        0x0117 => Some("Strip Byte Counts"),
        0x0118 => Some("Minimum Sample Value"),
        0x0119 => Some("Maximum Sample Value"),
        0x011a => Some("X Resolution"),
        0x011b => Some("Y Resolution"),
        0x011c => Some("Planar Configuration"),
        0x011d => Some("Page Name"),
        0x0128 => Some("Resolution Unit"),
        0x0129 => Some("Page Number"),
        0x012d => Some("Transfer Function"),
        0x0131 => Some("Software"),
        0x0132 => Some("Date/Time"),
        0x013b => Some("Artist"),
        0x013c => Some("Host Computer"),
        0x013d => Some("Predictor"),
        0x013e => Some("White Point"),
        0x013f => Some("Primary Chromaticities"),
        0x0142 => Some("Tile Width"),
        0x0143 => Some("Tile Length"),
        0x0144 => Some("Tile Offsets"),
        0x0145 => Some("Tile Byte Counts"),
        0x014a => Some("Sub IFD Pointer(s)"),
        0x0152 => Some("Extra Samples"),
        0x0153 => Some("Sample Format"),
        0x0156 => Some("Transfer Range"),
        0x015b => Some("JPEG Tables"),
        0x0200 => Some("JPEG Proc"),
        0x0203 => Some("JPEG Restart Interval"),
        0x0205 => Some("JPEG Lossless Predictors"),
        0x0206 => Some("JPEG Point Transforms"),
        0x0207 => Some("JPEGQ Tables"),
        0x0208 => Some("JPEGDC Tables"),
        0x0209 => Some("JPEGAC Tables"),
        0x0211 => Some("YCbCr Coefficients"),
        0x0212 => Some("YCbCr Sub-Sampling"),
        0x0213 => Some("YCbCr Positioning"),
        0x0214 => Some("Reference Black/White"),
        0x022f => Some("Strip Row Counts"),
        0x02bc => Some("Application Notes"),
        0x1000 => Some("Related Image File Format"),
        0x1001 => Some("Related Image Width"),
        0x1002 => Some("Related Image Height"),
        0x4746 => Some("Rating"),
        0x4749 => Some("Rating Percent"),
        0x828d => Some("CFA Repeat Pattern Dim"),
        0x828e => Some("CFA Pattern"),
        0x828f => Some("Battery Level"),
        0x8298 => Some("Copyright"),
        0x829a => Some("Exposure Time"),
        0x829d => Some("F-Number"),
        0x83bb => Some("IPTC/NAA"),
        0x8649 => Some("Photoshop Settings"),
        0x8773 => Some("Inter Color Profile"),
        0x8822 => Some("Exposure Program"),
        0x8824 => Some("Spectral Sensitivity"),
        0x8827 => Some("ISO Speed Ratings"),
        0x8828 => Some("Opto-electric Conversion Function (OECF)"),
        0x8829 => Some("Interlace"),
        0x882a => Some("Time Zone Offset"),
        0x882b => Some("Self Timer Mode"),
        0x8830 => Some("Sensitivity Type"),
        0x8831 => Some("Standard Output Sensitivity"),
        0x8832 => Some("Recommended Exposure Index"),
        0x8833 => Some("ISO Speed"),
        0x8834 => Some("ISO Speed Latitude yyy"),
        0x8835 => Some("ISO Speed Latitude zzz"),
        0x9000 => Some("Exif Version"),
        0x9003 => Some("Date/Time Original"),
        0x9004 => Some("Date/Time Digitized"),
        0x9010 => Some("Time Zone"),
        0x9011 => Some("Time Zone Original"),
        0x9012 => Some("Time Zone Digitized"),
        0x9101 => Some("Components Configuration"),
        0x9102 => Some("Compressed Bits Per Pixel"),
        0x9201 => Some("Shutter Speed Value"),
        0x9202 => Some("Aperture Value"),
        0x9203 => Some("Brightness Value"),
        0x9204 => Some("Exposure Bias Value"),
        0x9205 => Some("Max Aperture Value"),
        0x9206 => Some("Subject Distance"),
        0x9207 => Some("Metering Mode"),
        0x9208 => Some("White Balance"),
        0x9209 => Some("Flash"),
        0x920a => Some("Focal Length"),
        0x920b => Some("Flash Energy"),
        0x920c => Some("Spatial Frequency Response"),
        0x920d => Some("Noise"),
        0x920e => Some("Focal Plane X Resolution"),
        0x920f => Some("Focal Plane Y Resolution"),
        0x9211 => Some("Image Number"),
        0x9212 => Some("Security Classification"),
        0x9213 => Some("Image History"),
        0x9214 => Some("Subject Location"),
        0x9215 => Some("Exposure Index"),
        0x9216 => Some("TIFF/EP Standard ID"),
        0x927c => Some("Makernote"),
        0x9286 => Some("User Comment"),
        0x9290 => Some("Sub-Sec Time"),
        0x9291 => Some("Sub-Sec Time Original"),
        0x9292 => Some("Sub-Sec Time Digitized"),
        0x9400 => Some("Temperature"),
        0x9401 => Some("Humidity"),
        0x9402 => Some("Pressure"),
        0x9403 => Some("Water Depth"),
        0x9404 => Some("Acceleration"),
        0x9405 => Some("Camera Elevation Angle"),
        0x9c9b => Some("Windows XP Title"),
        0x9c9c => Some("Windows XP Comment"),
        0x9c9d => Some("Windows XP Author"),
        0x9c9e => Some("Windows XP Keywords"),
        0x9c9f => Some("Windows XP Subject"),
        0xa000 => Some("FlashPix Version"),
        0xa001 => Some("Color Space"),
        0xa002 => Some("Exif Image Width"),
        0xa003 => Some("Exif Image Height"),
        0xa004 => Some("Related Sound File"),
        0xa20b => Some("Flash Energy"),
        0xa20c => Some("Spatial Frequency Response"),
        0xa20e => Some("Focal Plane X Resolution"),
        0xa20f => Some("Focal Plane Y Resolution"),
        0xa210 => Some("Focal Plane Resolution Unit"),
        0xa214 => Some("Subject Location"),
        0xa215 => Some("Exposure Index"),
        0xa217 => Some("Sensing Method"),
        0xa300 => Some("File Source"),
        0xa301 => Some("Scene Type"),
        0xa302 => Some("CFA Pattern"),
        0xa401 => Some("Custom Rendered"),
        0xa402 => Some("Exposure Mode"),
        0xa403 => Some("White Balance Mode"),
        0xa404 => Some("Digital Zoom Ratio"),
        0xa405 => Some("Focal Length 35"),
        0xa406 => Some("Scene Capture Type"),
        0xa407 => Some("Gain Control"),
        0xa408 => Some("Contrast"),
        0xa409 => Some("Saturation"),
        0xa40a => Some("Sharpness"),
        0xa40b => Some("Device Setting Description"),
        0xa40c => Some("Subject Distance Range"),
        0xa420 => Some("Unique Image ID"),
        0xa430 => Some("Camera Owner Name"),
        0xa431 => Some("Body Serial Number"),
        0xa432 => Some("Lens Specification"),
        0xa433 => Some("Lens Make"),
        0xa434 => Some("Lens Model"),
        0xa435 => Some("Lens Serial Number"),
        0xa500 => Some("Gamma"),
        0xc4a5 => Some("Print Image Matching (PIM) Info"),
        0xc6d2 => Some("Panasonic Title"),
        0xc6d3 => Some("Panasonic Title (2)"),
        0xea1c => Some("Padding"),
        0xfdea => Some("Lens"),
        _ => None,
    }
}

pub(crate) fn gps_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x0000 => Some("GPS Version ID"),
        0x0001 => Some("GPS Latitude Ref"),
        0x0002 => Some("GPS Latitude"),
        0x0003 => Some("GPS Longitude Ref"),
        0x0004 => Some("GPS Longitude"),
        0x0005 => Some("GPS Altitude Ref"),
        0x0006 => Some("GPS Altitude"),
        0x0007 => Some("GPS Time-Stamp"),
        0x0008 => Some("GPS Satellites"),
        0x0009 => Some("GPS Status"),
        0x000a => Some("GPS Measure Mode"),
        0x000b => Some("GPS DOP"),
        0x000c => Some("GPS Speed Ref"),
        0x000d => Some("GPS Speed"),
        0x000e => Some("GPS Track Ref"),
        0x000f => Some("GPS Track"),
        0x0010 => Some("GPS Img Direction Ref"),
        0x0011 => Some("GPS Img Direction"),
        0x0012 => Some("GPS Map Datum"),
        0x0013 => Some("GPS Dest Latitude Ref"),
        0x0014 => Some("GPS Dest Latitude"),
        0x0015 => Some("GPS Dest Longitude Ref"),
        0x0016 => Some("GPS Dest Longitude"),
        0x0017 => Some("GPS Dest Bearing Ref"),
        0x0018 => Some("GPS Dest Bearing"),
        0x0019 => Some("GPS Dest Distance Ref"),
        0x001a => Some("GPS Dest Distance"),
        0x001b => Some("GPS Processing Method"),
        0x001c => Some("GPS Area Information"),
        0x001d => Some("GPS Date Stamp"),
        0x001e => Some("GPS Differential"),
        0x001f => Some("GPS Horizontal Positioning Error"),
        _ => None,
    }
}

pub(crate) fn wav_encoding(code: u16) -> &'static str {
    match code {
        0x0001 => "Microsoft PCM",
        0x0002 => "Microsoft ADPCM",
        0x0003 => "Microsoft IEEE float",
        0x0004 => "Compaq VSELP",
        0x0005 => "IBM CVSD",
        0x0006 => "Microsoft a-Law",
        0x0007 => "Microsoft u-Law",
        0x0008 => "Microsoft DTS",
        0x0009 => "DRM",
        0x000a => "WMA 9 Speech",
        0x000b => "Microsoft Windows Media RT Voice",
        0x0010 => "OKI-ADPCM",
        0x0011 => "Intel IMA/DVI-ADPCM",
        0x0012 => "Videologic Mediaspace ADPCM",
        0x0013 => "Sierra ADPCM",
        0x0014 => "Antex G.723 ADPCM",
        0x0015 => "DSP Solutions DIGISTD",
        0x0016 => "DSP Solutions DIGIFIX",
        0x0017 => "Dialoic OKI ADPCM",
        0x0018 => "Media Vision ADPCM",
        0x0019 => "HP CU",
        0x001a => "HP Dynamic Voice",
        0x0020 => "Yamaha ADPCM",
        0x0021 => "SONARC Speech Compression",
        0x0022 => "DSP Group True Speech",
        0x0023 => "Echo Speech Corp.",
        0x0024 => "Virtual Music Audiofile AF36",
        0x0025 => "Audio Processing Tech.",
        0x0026 => "Virtual Music Audiofile AF10",
        0x0027 => "Aculab Prosody 1612",
        0x0028 => "Merging Tech. LRC",
        0x0030 => "Dolby AC2",
        0x0031 => "Microsoft GSM610",
        0x0032 => "MSN Audio",
        0x0033 => "Antex ADPCME",
        0x0034 => "Control Resources VQLPC",
        0x0035 => "DSP Solutions DIGIREAL",
        0x0036 => "DSP Solutions DIGIADPCM",
        0x0037 => "Control Resources CR10",
        0x0038 => "Natural MicroSystems VBX ADPCM",
        0x0039 => "Crystal Semiconductor IMA ADPCM",
        0x003a => "Echo Speech ECHOSC3",
        0x003b => "Rockwell ADPCM",
        0x003c => "Rockwell DIGITALK",
        0x003d => "Xebec Multimedia",
        0x0040 => "Antex G.721 ADPCM",
        0x0041 => "Antex G.728 CELP",
        0x0042 => "Microsoft MSG723",
        0x0043 => "IBM AVC ADPCM",
        0x0045 => "ITU-T G.726",
        0x0050 => "Microsoft MPEG",
        0x0051 => "RT23 or PAC",
        0x0052 => "InSoft RT24",
        0x0053 => "InSoft PAC",
        0x0055 => "MP3",
        0x0059 => "Cirrus",
        0x0060 => "Cirrus Logic",
        0x0061 => "ESS Tech. PCM",
        0x0062 => "Voxware Inc.",
        0x0063 => "Canopus ATRAC",
        0x0064 => "APICOM G.726 ADPCM",
        0x0065 => "APICOM G.722 ADPCM",
        0x0066 => "Microsoft DSAT",
        0x0067 => "Micorsoft DSAT DISPLAY",
        0x0069 => "Voxware Byte Aligned",
        0x0070 => "Voxware AC8",
        0x0071 => "Voxware AC10",
        0x0072 => "Voxware AC16",
        0x0073 => "Voxware AC20",
        0x0074 => "Voxware MetaVoice",
        0x0075 => "Voxware MetaSound",
        0x0076 => "Voxware RT29HW",
        0x0077 => "Voxware VR12",
        0x0078 => "Voxware VR18",
        0x0079 => "Voxware TQ40",
        0x007a => "Voxware SC3",
        0x007b => "Voxware SC3",
        0x0080 => "Soundsoft",
        0x0081 => "Voxware TQ60",
        0x0082 => "Microsoft MSRT24",
        0x0083 => "AT&T G.729A",
        0x0084 => "Motion Pixels MVI MV12",
        0x0085 => "DataFusion G.726",
        0x0086 => "DataFusion GSM610",
        0x0088 => "Iterated Systems Audio",
        0x0089 => "Onlive",
        0x008a => "Multitude, Inc. FT SX20",
        0x008b => "Infocom ITS A/S G.721 ADPCM",
        0x008c => "Convedia G729",
        0x008d => "Not specified congruency, Inc.",
        0x0091 => "Siemens SBC24",
        0x0092 => "Sonic Foundry Dolby AC3 APDIF",
        0x0093 => "MediaSonic G.723",
        0x0094 => "Aculab Prosody 8kbps",
        0x0097 => "ZyXEL ADPCM",
        0x0098 => "Philips LPCBB",
        0x0099 => "Studer Professional Audio Packed",
        0x00a0 => "Malden PhonyTalk",
        0x00a1 => "Racal Recorder GSM",
        0x00a2 => "Racal Recorder G720.a",
        0x00a3 => "Racal G723.1",
        0x00a4 => "Racal Tetra ACELP",
        0x00b0 => "NEC AAC NEC Corporation",
        0x00ff => "AAC",
        0x0100 => "Rhetorex ADPCM",
        0x0101 => "IBM u-Law",
        0x0102 => "IBM a-Law",
        0x0103 => "IBM ADPCM",
        0x0111 => "Vivo G.723",
        0x0112 => "Vivo Siren",
        0x0120 => "Philips Speech Processing CELP",
        0x0121 => "Philips Speech Processing GRUNDIG",
        0x0123 => "Digital G.723",
        0x0125 => "Sanyo LD ADPCM",
        0x0130 => "Sipro Lab ACEPLNET",
        0x0131 => "Sipro Lab ACELP4800",
        0x0132 => "Sipro Lab ACELP8V3",
        0x0133 => "Sipro Lab G.729",
        0x0134 => "Sipro Lab G.729A",
        0x0135 => "Sipro Lab Kelvin",
        0x0136 => "VoiceAge AMR",
        0x0140 => "Dictaphone G.726 ADPCM",
        0x0150 => "Qualcomm PureVoice",
        0x0151 => "Qualcomm HalfRate",
        0x0155 => "Ring Zero Systems TUBGSM",
        0x0160 => "Microsoft Audio1",
        0x0161 => "Windows Media Audio V2 V7 V8 V9 / DivX audio (WMA) / Alex AC3 Audio",
        0x0162 => "Windows Media Audio Professional V9",
        0x0163 => "Windows Media Audio Lossless V9",
        0x0164 => "WMA Pro over S/PDIF",
        0x0170 => "UNISYS NAP ADPCM",
        0x0171 => "UNISYS NAP ULAW",
        0x0172 => "UNISYS NAP ALAW",
        0x0173 => "UNISYS NAP 16K",
        0x0174 => "MM SYCOM ACM SYC008 SyCom Technologies",
        0x0175 => "MM SYCOM ACM SYC701 G726L SyCom Technologies",
        0x0176 => "MM SYCOM ACM SYC701 CELP54 SyCom Technologies",
        0x0177 => "MM SYCOM ACM SYC701 CELP68 SyCom Technologies",
        0x0178 => "Knowledge Adventure ADPCM",
        0x0180 => "Fraunhofer IIS MPEG2AAC",
        0x0190 => "Digital Theater Systems DTS DS",
        0x0200 => "Creative Labs ADPCM",
        0x0202 => "Creative Labs FASTSPEECH8",
        0x0203 => "Creative Labs FASTSPEECH10",
        0x0210 => "UHER ADPCM",
        0x0215 => "Ulead DV ACM",
        0x0216 => "Ulead DV ACM",
        0x0220 => "Quarterdeck Corp.",
        0x0230 => "I-Link VC",
        0x0240 => "Aureal Semiconductor Raw Sport",
        0x0241 => "ESST AC3",
        0x0250 => "Interactive Products HSX",
        0x0251 => "Interactive Products RPELP",
        0x0260 => "Consistent CS2",
        0x0270 => "Sony SCX",
        0x0271 => "Sony SCY",
        0x0272 => "Sony ATRAC3",
        0x0273 => "Sony SPC",
        0x0280 => "TELUM Telum Inc.",
        0x0281 => "TELUMIA Telum Inc.",
        0x0285 => "Norcom Voice Systems ADPCM",
        0x0300 => "Fujitsu FM TOWNS SND",
        0x0301 => "Fujitsu (not specified)",
        0x0302 => "Fujitsu (not specified)",
        0x0303 => "Fujitsu (not specified)",
        0x0304 => "Fujitsu (not specified)",
        0x0305 => "Fujitsu (not specified)",
        0x0306 => "Fujitsu (not specified)",
        0x0307 => "Fujitsu (not specified)",
        0x0308 => "Fujitsu (not specified)",
        0x0350 => "Micronas Semiconductors, Inc. Development",
        0x0351 => "Micronas Semiconductors, Inc. CELP833",
        0x0400 => "Brooktree Digital",
        0x0401 => "Intel Music Coder (IMC)",
        0x0402 => "Ligos Indeo Audio",
        0x0450 => "QDesign Music",
        0x0500 => "On2 VP7 On2 Technologies",
        0x0501 => "On2 VP6 On2 Technologies",
        0x0680 => "AT&T VME VMPCM",
        0x0681 => "AT&T TCP",
        0x0700 => "YMPEG Alpha (dummy for MPEG-2 compressor)",
        0x08ae => "ClearJump LiteWave (lossless)",
        0x1000 => "Olivetti GSM",
        0x1001 => "Olivetti ADPCM",
        0x1002 => "Olivetti CELP",
        0x1003 => "Olivetti SBC",
        0x1004 => "Olivetti OPR",
        0x1100 => "Lernout & Hauspie",
        0x1101 => "Lernout & Hauspie CELP codec",
        0x1102 => "Lernout & Hauspie SBC codec",
        0x1103 => "Lernout & Hauspie SBC codec",
        0x1104 => "Lernout & Hauspie SBC codec",
        0x1400 => "Norris Comm. Inc.",
        0x1401 => "ISIAudio",
        0x1500 => "AT&T Soundspace Music Compression",
        0x181c => "VoxWare RT24 speech codec",
        0x181e => "Lucent elemedia AX24000P Music codec",
        0x1971 => "Sonic Foundry LOSSLESS",
        0x1979 => "Innings Telecom Inc. ADPCM",
        0x1c07 => "Lucent SX8300P speech codec",
        0x1c0c => "Lucent SX5363S G.723 compliant codec",
        0x1f03 => "CUseeMe DigiTalk (ex-Rocwell)",
        0x1fc4 => "NCT Soft ALF2CD ACM",
        0x2000 => "FAST Multimedia DVM",
        0x2001 => "Dolby DTS (Digital Theater System)",
        0x2002 => "RealAudio 1 / 2 14.4",
        0x2003 => "RealAudio 1 / 2 28.8",
        0x2004 => "RealAudio G2 / 8 Cook (low bitrate)",
        0x2005 => "RealAudio 3 / 4 / 5 Music (DNET)",
        0x2006 => "RealAudio 10 AAC (RAAC)",
        0x2007 => "RealAudio 10 AAC+ (RACP)",
        0x2500 => "Reserved range to 0x2600 Microsoft",
        0x3313 => "makeAVIS (ffvfw fake AVI sound from AviSynth scripts)",
        0x4143 => "Divio MPEG-4 AAC audio",
        0x4201 => "Nokia adaptive multirate",
        0x4243 => "Divio G726 Divio, Inc.",
        0x434c => "LEAD Speech",
        0x564c => "LEAD Vorbis",
        0x5756 => "WavPack Audio",
        0x674f => "Ogg Vorbis (mode 1)",
        0x6750 => "Ogg Vorbis (mode 2)",
        0x6751 => "Ogg Vorbis (mode 3)",
        0x676f => "Ogg Vorbis (mode 1+)",
        0x6770 => "Ogg Vorbis (mode 2+)",
        0x6771 => "Ogg Vorbis (mode 3+)",
        0x7000 => "3COM NBX 3Com Corporation",
        0x706d => "FAAD AAC",
        0x7a21 => "GSM-AMR (CBR, no SID)",
        0x7a22 => "GSM-AMR (VBR, including SID)",
        0xa100 => "Comverse Infosys Ltd. G723 1",
        0xa101 => "Comverse Infosys Ltd. AVQSBC",
        0xa102 => "Comverse Infosys Ltd. OLDSBC",
        0xa103 => "Symbol Technologies G729A",
        0xa104 => "VoiceAge AMR WB VoiceAge Corporation",
        0xa105 => "Ingenient Technologies Inc. G726",
        0xa106 => "ISO/MPEG-4 advanced audio Coding",
        0xa107 => "Encore Software Ltd G726",
        0xa109 => "Speex ACM Codec xiph.org",
        0xdfac => "DebugMode SonicFoundry Vegas FrameServer ACM Codec",
        0xe708 => "Unknown -",
        0xf1ac => "Free Lossless Audio Codec FLAC",
        0xfffe => "Extensible",
        0xffff => "Development",
        _ => "Unknown",
    }
}

pub(crate) fn exif_special(tag: u16) -> bool {
    matches!(
        tag,
        0x0001
            | 0x0002
            | 0x00fe
            | 0x00ff
            | 0x0100
            | 0x0101
            | 0x0102
            | 0x0103
            | 0x0106
            | 0x0107
            | 0x010a
            | 0x0112
            | 0x0115
            | 0x0116
            | 0x0117
            | 0x011a
            | 0x011b
            | 0x011c
            | 0x0128
            | 0x0152
            | 0x0153
            | 0x0200
            | 0x0212
            | 0x0213
            | 0x0214
            | 0x828e
            | 0x829a
            | 0x829d
            | 0x8822
            | 0x8827
            | 0x8830
            | 0x9000
            | 0x9101
            | 0x9102
            | 0x9201
            | 0x9202
            | 0x9203
            | 0x9204
            | 0x9205
            | 0x9206
            | 0x9207
            | 0x9208
            | 0x9209
            | 0x920a
            | 0x9286
            | 0x9400
            | 0x9401
            | 0x9402
            | 0x9403
            | 0x9404
            | 0x9405
            | 0x9c9b
            | 0x9c9c
            | 0x9c9d
            | 0x9c9e
            | 0x9c9f
            | 0xa000
            | 0xa001
            | 0xa002
            | 0xa003
            | 0xa20e
            | 0xa20f
            | 0xa210
            | 0xa217
            | 0xa300
            | 0xa301
            | 0xa302
            | 0xa401
            | 0xa402
            | 0xa403
            | 0xa404
            | 0xa405
            | 0xa406
            | 0xa407
            | 0xa408
            | 0xa409
            | 0xa40a
            | 0xa40c
            | 0xa432
    )
}

pub(crate) fn gps_special(tag: u16) -> bool {
    matches!(
        tag,
        0x0000
            | 0x0002
            | 0x0004
            | 0x0005
            | 0x0006
            | 0x0007
            | 0x0009
            | 0x000a
            | 0x000b
            | 0x000c
            | 0x000d
            | 0x000e
            | 0x000f
            | 0x0010
            | 0x0011
            | 0x0014
            | 0x0016
            | 0x0017
            | 0x0018
            | 0x0019
            | 0x001a
            | 0x001b
            | 0x001c
            | 0x001e
            | 0x001f
    )
}

pub(crate) fn exif_indexed(tag: u16, value: i64) -> Option<&'static str> {
    match (tag, value) {
        (0x00fe, 0) => Some("Full-resolution image"),
        (0x00fe, 1) => Some("Reduced-resolution image"),
        (0x00fe, 2) => Some("Single page of multi-page image"),
        (0x00fe, 3) => Some("Single page of multi-page reduced-resolution image"),
        (0x00fe, 4) => Some("Transparency mask"),
        (0x00fe, 5) => Some("Transparency mask of reduced-resolution image"),
        (0x00fe, 6) => Some("Transparency mask of multi-page image"),
        (0x00fe, 7) => Some("Transparency mask of reduced-resolution multi-page image"),
        (0x00ff, 1) => Some("Full-resolution image"),
        (0x00ff, 2) => Some("Reduced-resolution image"),
        (0x00ff, 3) => Some("Single page of multi-page image"),
        (0x0107, 1) => Some("No dithering or halftoning"),
        (0x0107, 2) => Some("Ordered dither or halftone"),
        (0x0107, 3) => Some("Randomized dither"),
        (0x010a, 1) => Some("Normal"),
        (0x010a, 2) => Some("Reversed"),
        (0x011c, 1) => Some("Chunky (contiguous for each subsampling pixel)"),
        (0x011c, 2) => Some("Separate (Y-plane/Cb-plane/Cr-plane format)"),
        (0x0128, 1) => Some("(No unit)"),
        (0x0128, 2) => Some("Inch"),
        (0x0128, 3) => Some("cm"),
        (0x0152, 0) => Some("Unspecified"),
        (0x0152, 1) => Some("Associated alpha"),
        (0x0152, 2) => Some("Unassociated alpha"),
        (0x0213, 1) => Some("Center of pixel array"),
        (0x0213, 2) => Some("Datum point"),
        (0x8822, 1) => Some("Manual control"),
        (0x8822, 2) => Some("Program normal"),
        (0x8822, 3) => Some("Aperture priority"),
        (0x8822, 4) => Some("Shutter priority"),
        (0x8822, 5) => Some("Program creative (slow program)"),
        (0x8822, 6) => Some("Program action (high-speed program)"),
        (0x8822, 7) => Some("Portrait mode"),
        (0x8822, 8) => Some("Landscape mode"),
        (0x8830, 0) => Some("Unknown"),
        (0x8830, 1) => Some("Standard Output Sensitivity"),
        (0x8830, 2) => Some("Recommended Exposure Index"),
        (0x8830, 3) => Some("ISO Speed"),
        (0x8830, 4) => Some("Standard Output Sensitivity and Recommended Exposure Index"),
        (0x8830, 5) => Some("Standard Output Sensitivity and ISO Speed"),
        (0x8830, 6) => Some("Recommended Exposure Index and ISO Speed"),
        (0x8830, 7) => {
            Some("Standard Output Sensitivity, Recommended Exposure Index and ISO Speed")
        }
        (0xa210, 1) => Some("(No unit)"),
        (0xa210, 2) => Some("Inches"),
        (0xa210, 3) => Some("cm"),
        (0xa217, 1) => Some("(Not defined)"),
        (0xa217, 2) => Some("One-chip color area sensor"),
        (0xa217, 3) => Some("Two-chip color area sensor"),
        (0xa217, 4) => Some("Three-chip color area sensor"),
        (0xa217, 5) => Some("Color sequential area sensor"),
        (0xa217, 7) => Some("Trilinear sensor"),
        (0xa217, 8) => Some("Color sequential linear sensor"),
        (0xa300, 1) => Some("Film Scanner"),
        (0xa300, 2) => Some("Reflection Print Scanner"),
        (0xa300, 3) => Some("Digital Still Camera (DSC)"),
        (0xa301, 1) => Some("Directly photographed image"),
        (0xa401, 0) => Some("Normal process"),
        (0xa401, 1) => Some("Custom process"),
        (0xa402, 0) => Some("Auto exposure"),
        (0xa402, 1) => Some("Manual exposure"),
        (0xa402, 2) => Some("Auto bracket"),
        (0xa403, 0) => Some("Auto white balance"),
        (0xa403, 1) => Some("Manual white balance"),
        (0xa406, 0) => Some("Standard"),
        (0xa406, 1) => Some("Landscape"),
        (0xa406, 2) => Some("Portrait"),
        (0xa406, 3) => Some("Night scene"),
        (0xa407, 0) => Some("None"),
        (0xa407, 1) => Some("Low gain up"),
        (0xa407, 2) => Some("Low gain down"),
        (0xa407, 3) => Some("High gain up"),
        (0xa407, 4) => Some("High gain down"),
        (0xa408, 0) => Some("None"),
        (0xa408, 1) => Some("Soft"),
        (0xa408, 2) => Some("Hard"),
        (0xa409, 0) => Some("None"),
        (0xa409, 1) => Some("Low saturation"),
        (0xa409, 2) => Some("High saturation"),
        (0xa40a, 0) => Some("None"),
        (0xa40a, 1) => Some("Low"),
        (0xa40a, 2) => Some("Hard"),
        (0xa40c, 0) => Some("Unknown"),
        (0xa40c, 1) => Some("Macro"),
        (0xa40c, 2) => Some("Close view"),
        (0xa40c, 3) => Some("Distant view"),
        _ => None,
    }
}

pub(crate) fn exif_indexed_tag(tag: u16) -> bool {
    matches!(
        tag,
        0x00fe
            | 0x00ff
            | 0x0107
            | 0x010a
            | 0x011c
            | 0x0128
            | 0x0152
            | 0x0213
            | 0x8822
            | 0x8830
            | 0xa210
            | 0xa217
            | 0xa300
            | 0xa301
            | 0xa401
            | 0xa402
            | 0xa403
            | 0xa406
            | 0xa407
            | 0xa408
            | 0xa409
            | 0xa40a
            | 0xa40c
    )
}

pub(crate) fn gps_indexed(tag: u16, value: i64) -> Option<&'static str> {
    match (tag, value) {
        (0x0005, 0) => Some("Sea level"),
        (0x0005, 1) => Some("Below sea level"),
        (0x001e, 0) => Some("No Correction"),
        (0x001e, 1) => Some("Differential Corrected"),
        _ => None,
    }
}

pub(crate) fn gps_indexed_tag(tag: u16) -> bool {
    matches!(tag, 0x0005 | 0x001e)
}

pub(crate) fn panasonic_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x0001 => Some("Panasonic Raw Version"),
        0x0002 => Some("Sensor Width"),
        0x0003 => Some("Sensor Height"),
        0x0004 => Some("Sensor Top Border"),
        0x0005 => Some("Sensor Left Border"),
        0x0006 => Some("Sensor Bottom Border"),
        0x0007 => Some("Sensor Right Border"),
        0x0008 => Some("Black Level 1"),
        0x0009 => Some("Black Level 2"),
        0x000a => Some("Black Level 3"),
        0x000e => Some("Linearity Limit Red"),
        0x000f => Some("Linearity Limit Green"),
        0x0010 => Some("Linearity Limit Blue"),
        0x0011 => Some("Red Balance"),
        0x0012 => Some("Blue Balance"),
        0x0017 => Some("ISO"),
        0x0018 => Some("High ISO Multiplier Red"),
        0x0019 => Some("High ISO Multiplier Green"),
        0x001a => Some("High ISO Multiplier Blue"),
        0x001c => Some("Black Level Red"),
        0x001d => Some("Black Level Green"),
        0x001e => Some("Black Level Blue"),
        0x0024 => Some("WB Red Level"),
        0x0025 => Some("WB Green Level"),
        0x0026 => Some("WB Blue Level"),
        0x002e => Some("Jpg From Raw"),
        0x002f => Some("Crop Top"),
        0x0030 => Some("Crop Left"),
        0x0031 => Some("Crop Bottom"),
        0x0032 => Some("Crop Right"),
        0x010f => Some("Make"),
        0x0110 => Some("Model"),
        0x0111 => Some("Strip Offsets"),
        0x0112 => Some("Orientation"),
        0x0116 => Some("Rows Per Strip"),
        0x0117 => Some("Strip Byte Counts"),
        0x0118 => Some("Raw Data Offset"),
        _ => None,
    }
}
