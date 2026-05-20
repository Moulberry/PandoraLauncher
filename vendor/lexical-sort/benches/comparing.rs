use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
};
use lexical_sort::{
    cmp, lexical_cmp, lexical_only_alnum_cmp, natural_cmp, natural_lexical_cmp,
    natural_lexical_only_alnum_cmp, natural_only_alnum_cmp, only_alnum_cmp,
};
use std::cmp::Ordering;

use rust_icu_ucol::UCollator;
use rust_icu_ustring::UChar;
use std::convert::TryFrom;

// 100 auto-generated strings with a length between 5 and 20 characters
//
// Half of them have another string with a common prefix:
//  - 50 strings whose first character is unique
//  - 12 pairs with a common prefix of length 1
//  - 6 pairs with a common prefix of length 2
//  - 2 pairs with a common prefix of length 3
//  - 2 pairs with a common prefix of length 4
//  - 2 pairs with a common prefix of length 6
//  - 1 pair with a common prefix of length 10 ("pŊcóh~öJL*fæL", "pŊcóh~öJL*Ł")
//
// The strings are shuffled randomly.
#[rustfmt::skip]
static STRINGS: [&str; 100] = [
    "dT@ŁeΩØä.µ#", "ŋ +GÐNSCEṇZæB+łßc", "Øí mÞt@Bwi", "+9Üyđg", "ZAgL!Gé",
    "NE€j̣đm¼ŁCBLWHjŊ", "zvħü¼Ø!iµ¼.", "ŁøŒSBN9", "(̣¥AŒvŁy40", "æ#áBV(AŁFAdQ. 8",
    "#½IṣÞY²̣", "9f1kẞIz7æ3€ßeŁ", "ŧywΩEc", "!½j.œ³d⅞qÖDVÜðQNŒ", "Äþ+RF ",
    "¼µírT²ałDKnwB.T8P", "€ŋØáEIJt @éµ", "³3ömrn@H2jóĸ#R", "Ŋđ⅞y$VṚ)ĦE",
    "ŧF€zÐxLúT7²ẒXŧ̣", "ÜDµ̣LNĸd²Öh€Æn~̣", "ä1v9⅞$ĸ⅝Äh.91Þ.#", "Ló1vó ḄZkḍíT€",
    "½5uØeámΩ", "ZAþmD²VfHṂe", "1cŒCu8f½¼4ÞáeŒΩq", "F+úÖsI½f#n7GfV#Æ¼",
    "TÆạÜGbUjſfDnŁ", "Uþĸ 7Ä$öáx⅝ävyU", " ÄÐEØ*", "Ω9äülq6H", "Ω9äülq.o",
    "E1Ö0AD5̣SM(̣", "QvlkRiTŋúÐa", " ̣yZ$!ÄZ¥", "$1pđÞúþĦ¼GSWħØW", "úð5 +ẞŊÞi1ðTðŋ",
    "!½j.BΩ*", "tpHV$eḳ", "7 ⅝ Eiæ#̣aŊum³3ŊR", "iJ̣P)F", "½plŒügäGŊFħPł3¹Ö8Ω",
    "BĦ$Œö8¹̣úH", "Ħæä3v³nzÖc", "Iøþænóœ", "qi⅝qzH ⅝", "Rg@W6Æ0ịI9¹h)", "fÖ¼Æ¼üÜ²",
    "3ΩŒŁá²hŊOZ½AŁndt", "F1Ðøéu½áR", "̣lzłäÞ", "yſ8hẞ¹²zọ́ry", "ßímJĸ+", "Ðr1LwO²øvØ",
    "Ä2a0DÜvMØΩ)íÆ9Ŋ̣", "8þJ⅝UÞΩ", "Æ²ḲŧĦFGÐŊä$s83h", "DGbŁbœfP", "kHö9̣xps b!yOlV#",
    "~+yĦ$³LI", "nTVT²Þµ!Œ", "Ppyg̣Ð.LlYĸẞħ", "µ⅝4CŁQBQp", "ü!HßÞéGzøED@€€",
    "W⅞i.C3Ṛþ", "fÖØ+d1ä", "euA)WE", "DGbŁÖé8!j!DBł9a", "wsRCQßA", "ÐrẠMjdtaj*",
    "ł+¥UŊiÆiÜé3̣Bím", "PpÖụXéẞiLµNHLCÄ", "ég³$ØOxqfbUḲðy L", "pŊcóh~öJL*fæL",
    "8O#SOÞ̣b+⅞ĸ!W!⅞pc", "1cpb6wſPßS2ŒĦ", "Þvð¼ÖbvWf!Oüđ*Zħ5", "JeníéCp*Ö",
    "pŊcóh~öJL*Ł", "i!5ZQ", "E1ÖRŊ€dœ+.Þ", "Hŧ7$SáĦt8B", "yt½Xđm", "ŒE¼p̣N8²",
    "⅞eþ NFáŒ̣+JÄLrQs", "OøqKar~ü", "ÖĸΩÖ ", "hpoXt", "7$eŁPħØ2#AßµµđÐí+Y",
    "đØ@þ³v*¹Bco8µŊhw", "wsRCQß̣*Cg", "#ZvIΩłKẞØ7s~~1x", "²øeXbl3PrŋŁBE1m", "ọOhŁó",
    "nT #2€9ł l", "2XöáUbúeV", "µ⅝40uVøQUD+̣", "YdBycð@ọ́D!¼cp8", "dünÆéÆ@µZ!f+",
    "OJoÄXPŊ",
];

// 100 strings containing the prefix "T-", followed by a number
// of random length between 1 and 8 digits.
#[rustfmt::skip]
static NUM_STRINGS: [&str; 100] = [
    "T-60575", "T-49", "T-91237", "T-31512", "T-12043", "T-3012138", "T-2008720",
    "T-160", "T-111246", "T-19104", "T-1948241", "T-10130396", "T-1103", "T-5067",
    "T-53403082", "T-66774788", "T-15", "T-162166", "T-3774328", "T-5", "T-248913",
    "T-202", "T-521749", "T-125", "T-7631192", "T-38", "T-318", "T-639390", "T-157",
    "T-61", "T-3798339", "T-27802", "T-14384042", "T-12229308", "T-2", "T-3903",
    "T-89476", "T-112", "T-744113", "T-236", "T-20137", "T-23527", "T-4810796",
    "T-11", "T-354", "T-8594", "T-2", "T-12076", "T-1206524", "T-3554909", "T-1",
    "T-12", "T-49366", "T-16249281", "T-51", "T-472", "T-1586", "T-16112257", "T-12",
    "T-139", "T-14624", "T-110", "T-16603", "T-5254907", "T-61", "T-8055", "T-44",
    "T-11", "T-111", "T-369", "T-64009992", "T-7", "T-515833", "T-3", "T-51",
    "T-122075", "T-13", "T-1669", "T-561456", "T-252344", "T-9371073", "T-361096",
    "T-5252431", "T-33587", "T-2", "T-5098095", "T-182", "T-3568008", "T-11559",
    "T-13", "T-653", "T-1085", "T-13249", "T-932722", "T-3036859", "T-52065", "T-23189",
    "T-114", "T-40716", "T-145243",
];

// 100 auto-generated ASCII-only strings with a length between 5 and 20 characters
//
// Half of them have another string with a common prefix:
//  - 50 strings whose first character is unique
//  - 12 pairs with a common prefix of length 1
//  - 6 pairs with a common prefix of length 2
//  - 2 pairs with a common prefix of length 3
//  - 2 pairs with a common prefix of length 4
//  - 2 pairs with a common prefix of length 6
//  - 1 pair with a common prefix of length 10 ("/l>Wvr<QV oaR", "/l>Wvr<QV |dv")
//
// The strings are shuffled randomly.
#[rustfmt::skip]
static ASCII_STRINGS: [&str; 100] = [
    "D'4e8uejiI\\P2-k", "f Yu|us 0oZH\"0 (\"3a", "\"uo%m", "[;}c-pw]?a*;4nf!rl",
    "\"usZj9;", "@)?38D:on\\ecm", "VM)k8R;U7exTnHo", "o&oui elo&aJjSX1,C", "W9F\\a",
    "2AnesdlTIei6V[h{Bnl", "AItIamru\\ x d", "tjnKB8a}Aqd>x8", "!duS ",
    "UfP6ej+j;H+Ssoizi", "aVb\\74#", "Xam^o:xm0l] mk2aM'", ";)Y2e", "yhmm['H^;aUqetA",
    ">sz@k?4u86sll2ZL", "P]([eGAocueN@+c%", "rovnD?)se", "Ffss \"0MFcPqo# KX8",
    ";)Ya)+i", ")Je6ush", "'ni.T8z.jzi;26age", "G*rMwN<f<5", "rE7mFm:kk# e&u XoaJ",
    "8oxso", "R }o5TlBn  hu y", "#h{eonvw,", "p dm#{i;q_", "bNeoxs:KR5u|a{4x",
    "k4bt\\JJnz]Ncp/Tef", "55N ]j|o2P0#}H", "DRwY#48(>/Ku w", "M:2pX88",
    "h ^e08TI.)TK", "'f6eh6", "jhM8rdi", "{8nVo", "7uP;}B", "+|1E!LWstxDGLM+X^",
    "Es|^4t)l", "WnF +lnF[iity\"Idc", "f Yu|u4", "if#QhodMf0e@}i", "uso t,", "mY c*2",
    ",U u?#4PN)T$G!e", "XaN<a1JDAutlfhSe", "Oein{q`bI@8Ut2&e)i", "{8_4uALXy moawe3Vz",
    "|'{2;\"", "%QcF<y] 84g44u", "czmQila ", "cz8J$", "IdnihN@iQmVtuT,",
    "jhM8rdilDaw5#lofh/k", "/l>Wvr<QV oaR", "za7IaK 8eR\"?t", "4U; icn#|dnf)o",
    "-s0ok", "*Pevdve6g[|H0c{xRf", "&bs2LmAdFrd]n2l", "<m73 ", "G*rMiue",
    "u'r?jseh|aIyfhe", " Gg49ozfr}K<uH0?n", "01Sneh%KQ`|", "k4Wna1.UnUA",
    "nPi{#@VGvS&", "ChFci|osP0=an0Qrh\\n", "\\(Ptlm*Lca", "!duGhCs91Iio4sbgZ>m",
    "#Bkv+i%6 qi8  ivl{", "3o0SXbo", "[;$_!;m4ylij`un-L", "su,Eu2Natgi S66zmT",
    "TC[p5hdfht{zrEi", "_z5ahCaN4XSS3", "7uP;WAPFf@'4q<m", "Bs588", "/l>Wvr<QV |dv",
    "Hl<aREd`_e2o@Bo", "d2-JsoSOnom](", "v%XYZ", "}nc9K$;", "+iCjAP uf)vh?s",
    "$%`\\0`sr!sPI@2s", "g04sEt),oEql1efs4", "IikiM\"kQ2zqA9V)<u", "i;^5:n^6",
    "62#lw{5o", "=buf2noe8wzdBT", "q`5cl8F]-4e_ DnK", "&#6/]`\\:hDsk",
    "elvdTcu.uf+a_W?Rd\\j", "VN4r>2E6<v(esGn", "EBcp*BMN;$lDsn", "]6muXiTau+K)y",
];

// Compare every string once with every string except itself
#[inline(always)]
fn for_all<'a, F>(arr: &'a [&'a str], f: F)
where
    F: Fn(&'a str, &'a str) -> Ordering,
{
    for i in 0..100_usize {
        for j in (i..100).skip(1) {
            let l = black_box(arr[i]);
            let r = black_box(arr[j]);
            black_box(f(l, r));
        }
    }
}

// Compare every string once with every string except itself
#[inline(always)]
fn for_all_s<'a, F>(arr: &'a [&'a str], f: F)
where
    F: Fn(&UChar, &UChar) -> Ordering,
{
    let v: Vec<(&str, UChar)> = arr
        .iter()
        .map(|&s| (s, UChar::try_from(s).unwrap()))
        .collect();

    for i in 0..100_usize {
        for j in (i..100).skip(1) {
            let l = black_box(&v[i]);
            let r = black_box(&v[j]);
            black_box(f(&l.1, &r.1));
        }
    }
}

fn bench_all_functions(group: &mut BenchmarkGroup<WallTime>, strs: &[&str; 100]) {
    group.bench_function("native (std)", |b| {
        b.iter(|| for_all(strs, str::cmp));
    });
    group.bench_function("natural (alphanumerical-sort)", |b| {
        b.iter(|| for_all(strs, alphanumeric_sort::compare_str));
    });
    group.bench_function("cmp", |b| {
        b.iter(|| for_all(strs, cmp));
    });
    group.bench_function("only alnum", |b| {
        b.iter(|| for_all(strs, only_alnum_cmp));
    });
    group.bench_function("lexical", |b| {
        b.iter(|| for_all(strs, lexical_cmp));
    });
    group.bench_function("lexical + only alnum", |b| {
        b.iter(|| for_all(strs, lexical_only_alnum_cmp));
    });
    group.bench_function("natural", |b| {
        b.iter(|| for_all(strs, natural_cmp));
    });
    group.bench_function("natural + only alnum", |b| {
        b.iter(|| for_all(strs, natural_only_alnum_cmp));
    });
    group.bench_function("natural + lexical", |b| {
        b.iter(|| for_all(strs, natural_lexical_cmp));
    });
    group.bench_function("natural + lexical + only alnum", |b| {
        b.iter(|| for_all(strs, natural_lexical_only_alnum_cmp));
    });

    let collator = UCollator::try_from("en").expect("collator");
    group.bench_function("professional", |b| {
        b.iter(|| for_all_s(strs, |a, b| collator.strcoll(a, b)));
    });
}

pub fn compare_strings(c: &mut Criterion) {
    let mut group = c.benchmark_group("Unicode strings");
    bench_all_functions(&mut group, &STRINGS);
    group.finish();
}

pub fn compare_ascii(c: &mut Criterion) {
    let mut group = c.benchmark_group("ASCII strings");
    bench_all_functions(&mut group, &ASCII_STRINGS);
    group.finish();
}

pub fn compare_numbers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Strings with numbers");
    bench_all_functions(&mut group, &NUM_STRINGS);
    group.finish();
}

criterion_group!(comparing, compare_strings, compare_ascii, compare_numbers);
criterion_main!(comparing);
