use serenity::model::id::UserId;

const ALL_NAMES: usize = FIRST_NAMES.len() * LAST_NAMES.len();
// Get a name from the user's id.
pub fn name_from_userid(u: UserId) -> (&'static str, &'static str) {
    let u = u.0 as usize % ALL_NAMES;
    (
        FIRST_NAMES[u / LAST_NAMES.len()], // Not your standard mod
        LAST_NAMES[u % LAST_NAMES.len()],
    )
}

const FIRST_NAMES: [&'static str; 440] = [
    // A Female names
    "Ai",
    "Aiko",
    "Akane",
    "Akemi",
    "Aki",
    "Akiko",
    "Akina",
    "Akuro",
    "Amarante",
    "Amaya",
    "Ami",
    "Anda",
    "Aneko",
    "Arisa",
    "Asako",
    "Asami",
    "Atsuko",
    "Aya",
    "Ayaka",
    "Ayako",
    "Ayame",
    "Ayano",
    // B Female names
    "Benten",
    // C Female names
    "Chiaki",
    "Chie",
    "Chieko",
    "Chihiro",
    "Chika",
    "Chikako",
    "Chiko",
    "Chikuma",
    "Chinatsu",
    "Chisaki",
    "Chisato",
    "Chitose",
    "Chiyeko",
    "Chiyo",
    "Cho",
    // D Female names
    "Dai",
    // E Female names
    "Echiko",
    "Eiko",
    "Ema",
    "Emi",
    "Emiko",
    "Eri",
    "Eriko",
    "Etsuko",
    "Euiko",
    // F Female names
    "Fujiko",
    "Fumi",
    "Fumie",
    "Fumiki",
    "Fumiko",
    "Fusae",
    "Fuyuko",
    // G Female names
    "Gemmei",
    "Gen",
    "Gin",
    "Ginko",
    // H Female names
    "Hama",
    "Hana",
    "Hanae",
    "Hanako",
    "Haniko",
    "Haru",
    "Haru",
    "Haruhi",
    "Haruka",
    "Harukichi",
    "Haruko",
    "Harumi",
    "Haruna",
    "Hatsue",
    "Hatsuyo",
    "Hide",
    "Hideko",
    "Hikaru",
    "Hiroe",
    "Hiroko",
    "Hiromi",
    "Hiroshi",
    "Hisa",
    "Hisae",
    "Hisako",
    "Hisano",
    "Hitomi",
    "Hitomo",
    "Hitoshi",
    "Honami",
    "Hoshi",
    "Hoshie",
    "Hoshiko",
    "Hoshiyo",
    // I Female names
    "Ichi",
    "Iku",
    "Ikue",
    "Ikuko",
    "Inari",
    "Inoue",
    "Isako",
    "Ise",
    "Itsuko",
    "Izumi",
    // J Female names
    "Jin",
    "Joruri",
    "Jun",
    "Junko",
    "Juri",
    // K Female names
    "Kaede",
    "Kagami",
    "Kahori",
    "Kaida",
    "Kaiya",
    "Kaiyo",
    "Kameko",
    "Kami",
    "Kammi",
    "Kammie",
    "Kana",
    "Kanami",
    "Kaneko",
    "Kaori",
    "Kaoru",
    "Kasuga",
    "Kata",
    "Katsue",
    "Katsuko",
    "Katsumi",
    "Kaya",
    "Kayoko",
    "Kazue",
    "Kazuko",
    "Kazumi",
    "Kei",
    "Keiko",
    "Kichi",
    "Kiko",
    "Kikuko",
    "Kikyou",
    "Kimi",
    "Kimie",
    "Kimiko",
    "Kin",
    "Kinuko",
    "Kinuye",
    "Kinuyo",
    "Kioko",
    "Kiriko",
    "Kishi",
    "Kita",
    "Kiyo",
    "Kiyoko",
    "Kiyomi",
    "Kochiyo",
    "Kohana",
    "Koi",
    "Koiso",
    "Koken",
    "Koko",
    "Komachi",
    "Koto",
    "Kotono",
    "Kumi",
    "Kumiko",
    "Kuni",
    "Kunie",
    "Kuniko",
    "Kura",
    "Kuri",
    "Kyoko",
    // M Female names
    "Machi",
    "Machiko",
    "Madoka",
    "Mae",
    "Maeko",
    "Maemi",
    "Mai",
    "Maiko",
    "Maiya",
    "Maki",
    "Makiko",
    "Mako",
    "Mami",
    "Mamiko",
    "Mana",
    "Manami",
    "Mari",
    "Mariko",
    "Marise",
    "Maru",
    "Masae",
    "Masako",
    "Masumi",
    "Matsu",
    "Matsuko",
    "Maya",
    "Mayako",
    "Mayo",
    "Mayoko",
    "Mayu",
    "Mayuko",
    "Mayumi",
    "Megu",
    "Megumi",
    "Michi",
    "Michie",
    "Michiko",
    "Michiru",
    "Michiyo",
    "Midori",
    "Mieko",
    "Miho",
    "Mihoko",
    "Miiko",
    "Miki",
    "Miliko",
    "Mina",
    "Minako",
    "Minami",
    "Mineko",
    "Mino",
    "Mio",
    "Misa",
    "Misako",
    "Misato",
    "Mitsu",
    "Mitsuko",
    "Mitsuyo",
    "Miwako",
    "Miya",
    "Miyako",
    "Miyo",
    "Miyoko",
    "Miyoshi",
    "Mizuki",
    "Moeko",
    "Momoko",
    "Mura",
    "Mutsuko",
    "Mutsumi",
    // N Female names
    "Naho",
    "Nahoko",
    "Nami",
    "Nami",
    "Namie",
    "Namika",
    "Namiko",
    "Namiyo",
    "Nana",
    "Nanako",
    "Nanami",
    "Nao",
    "Naoko",
    "Naora",
    "Nari",
    "Nariko",
    "Naru",
    "Narumi",
    "Natsuko",
    "Natsumi",
    "Natsumi",
    "Nayoko",
    "Nene",
    "Nishi",
    "Nomi",
    "Nori",
    "Norie",
    "Noriko",
    "Nozomi",
    "Nyoko",
    // O Female names
    "Ochiyo",
    "Oharu",
    "Oki",
    "Okichi",
    "Okiku",
    "Omitsu",
    "Orino",
    "Otsu",
    "Otsune",
    // R Female names
    "Raicho",
    "Raku",
    "Ran",
    "Rei",
    "Reiko",
    "Remi",
    "Rie",
    "Rieko",
    "Rika",
    "Rikako",
    "Riku",
    "Rina",
    "Rinako",
    "Rini",
    "Risa",
    "Risako",
    "Ritsuko",
    "Romi",
    "Rui",
    "Rumiko",
    "Ruri",
    "Ruriko",
    "Ryoko",
    // S Female names
    "Sachi",
    "Sachiko",
    "Sada",
    "Sadako",
    "Sae",
    "Saeko",
    "Saito",
    "Sakamae",
    "Saki",
    "Sakiko",
    "Sakue",
    "Sakuko",
    "Sakura",
    "Sakurako",
    "Sakuro",
    "Sama",
    "Sanako",
    "Saori",
    "Sata",
    "Satoko",
    "Satomi",
    "Satu",
    "Sawako",
    "Saya",
    "Sayo",
    "Sayoko",
    "Sayuri",
    "Sei",
    "Seiko",
    "Seka",
    "Seki",
    "Sen",
    "Setsuko",
    "Shige",
    "Shika",
    "Shina",
    "Shino",
    "Shinobu",
    "Shioko",
    "Shiori",
    "Shizu",
    "Shizue",
    "Shizuka",
    "Shoken",
    "Shoko",
    "Sui",
    "Suki",
    "Suko",
    "Sumi",
    "Sumie",
    "Sumiko",
    "Suzu",
    "Suzue",
    "Suzume",
    "Suzuko",
    // T Female names
    "Tadako",
    "Tae",
    "Tai",
    "Taji",
    "Taka",
    "Takako",
    "Takara",
    "Tama",
    "Tamae",
    "Tamafune",
    "Tamaki",
    "Tamami",
    "Tami",
    "Tamika",
    "Tamiko",
    "Tamiyo",
    "Tanak",
    "Taniko",
    "Tansho",
    "Tara",
    "Taree",
    "Taura",
    "Taya",
    "Teruyo",
    "Toki",
    "Tokie",
    "Tokiko",
    "Tokiyo",
    "Toku",
    "Tomi",
    "Tomiko",
    "Tomoe",
    "Tomoko",
    "Tomomi",
    "Toshi",
    "Toshie",
    "Toshiko",
    "Toya",
    "Toyoko",
    "Tsuki",
    "Tsukiyama",
    "Tsuya",
    // U Female names
    "Ume",
    "Umeka",
    "Umeko",
    "Urako",
    "Usagi",
    "Uta",
    "Utako",
    // W Female names
    "Wattan",
    "Wazuka",
    // Y Female names
    "Yachi",
    "Yae",
    "Yaeko",
    "Yama",
    "Yasu",
    "Yasuko",
    "Yayoi",
    "Yodo",
    "Yoko",
    "Yori",
    "Yoriko",
    "Yoshe",
    "Yoshi",
    "Yoshike",
    "Yoshiko",
    "Yoshino",
    "Yu",
    "Yui",
    "Yuka",
    "Yukako",
    "Yukari",
    "Yuki",
    "Yukiko",
    "Yukiyo",
    "Yuko",
    "Yuma",
    "Yumako",
    "Yumi",
    "Yumiko",
    "Yuri",
    "Yuriko",
    "Yusuke",
];

const LAST_NAMES: [&'static str; 1051] = [
    // A Surnames
    "Abe",
    "Abukara",
    "Adachi",
    "Aibu",
    "Aida",
    "Aihara",
    "Aizawa",
    "Ajibana",
    "Akaike",
    "Akamatsu",
    "Akatsuka",
    "Akechi",
    "Akera",
    "Akimoto",
    "Akita",
    "Akiyama",
    "Akutagawa",
    "Amagawa",
    "Amaya",
    "Amori",
    "Anami",
    "Ando",
    "Anzai",
    "Aoki",
    "Arai",
    "Arakaki",
    "Arakawa",
    "Araki",
    "Arakida",
    "Arato",
    "Arihyoshi",
    "Arishima",
    "Arita",
    "Ariwa",
    "Ariwara",
    "Asahara",
    "Asahi",
    "Asai",
    "Asano",
    "Asanuma",
    "Asari",
    "Ashia",
    "Ashida",
    "Ashikaga",
    "Asuhara",
    "Atshushi",
    "Ayabe",
    "Ayabito",
    "Ayugai",
    "Azama",
    // C Surnames
    "Chiba",
    "Chikamatsu",
    "Chikanatsu",
    "Chino",
    "Chishu",
    "Choshi",
    // D Surnames
    "Daishi",
    "Dan",
    "Date",
    "Dazai",
    "Deguchi",
    "Deushi",
    "Doi",
    // E Surnames
    "Ebina",
    "Ebisawa",
    "Eda",
    "Egami",
    "Eguchi",
    "Ekiguchi",
    "Endo",
    "Endoso",
    "Enoki",
    "Enomoto",
    "Erizawa",
    "Eto",
    "Etsuko",
    "Ezakiya",
    // F Surnames
    "Fuchida",
    "Fuchizaki",
    "Fugunaga",
    "Fujii",
    "Fujikage",
    "Fujimaki",
    "Fujimoto",
    "Fujioka",
    "Fujishima",
    "Fujita",
    "Fujiwara",
    "Fukao",
    "Fukayama",
    "Fukazawa",
    "Fukuda",
    "Fukumitsu",
    "Fukumoto",
    "Fukunaka",
    "Fukuoka",
    "Fukusaku",
    "Fukushima",
    "Fukuyama",
    "Fukuzawa",
    "Fumihiko",
    "Funabashi",
    "Funaki",
    "Funakoshi",
    "Furuhata",
    "Furusawa",
    "Fuschida",
    "Fuse",
    "Futabatei",
    "Fuwa",
    // G Surnames
    "Gakusha",
    "Genda",
    "Genji",
    "Gensai",
    "Godo",
    "Goto",
    "Gushiken",
    // H Surnames
    "Haga",
    "Hagino",
    "Hagiwara",
    "Hakamada",
    "Hama",
    "Hamacho",
    "Hamada",
    "Hamaguchi",
    "Hamamoto",
    "Han",
    "Hanabusa",
    "Hanari",
    "Handa",
    "Hara",
    "Harada",
    "Haruguchi",
    "Hasegawa",
    "Hasekura",
    "Hashi",
    "Hashimoto",
    "Hasimoto",
    "Hatakeda",
    "Hatakeyama",
    "Hatayama",
    "Hatoyama",
    "Hattori",
    "Hayakawa",
    "Hayami",
    "Hayashi",
    "Hayashida",
    "Hayata",
    "Hayuata",
    "Hida",
    "Hidaka",
    "Hideaki",
    "Hideki",
    "Hideyoshi",
    "Higa",
    "Higashi",
    "Higashikuni",
    "Higashiyama",
    "Higo",
    "Higoshi",
    "Higuchi",
    "Hike",
    "Hino",
    "Hira",
    "Hiraga",
    "Hirai",
    "Hiraki",
    "Hirano",
    "Hiranuma",
    "Hiraoka",
    "Hirase",
    "Hirasi",
    "Hirata",
    "Hiratasuka",
    "Hirayama",
    "Hiro",
    "Hirose",
    "Hirota",
    "Hiroyuki",
    "Hisamatsu",
    "Hishida",
    "Hishikawa",
    "Hitomi",
    "Hiyama",
    "Hohki",
    "Hojo",
    "Hokusai",
    "Honami",
    "Honda",
    "Hori",
    "Horigome",
    "Horigoshi",
    "Horiuchi",
    "Horri",
    "Hoshino",
    "Hosokawa",
    "Hosokaya",
    "Hotate",
    "Hotta",
    "Hyata",
    "Hyobanshi",
    // I Surnames
    "Ibi",
    "Ibu",
    "Ibuka",
    "Ichigawa",
    "Ichihara",
    "Ichikawa",
    "Ichimonji",
    "Ichiro",
    "Ichisada",
    "Ichiyusai",
    "Idane",
    "Iemochi",
    "Ienari",
    "Iesada",
    "Ieyasu",
    "Ieyoshi",
    "Igarashi",
    "Ihara",
    "Ii",
    "Iida",
    "Iijima",
    "Iitaka",
    "Ijichi",
    "Ijiri",
    "Ikeda",
    "Ikina",
    "Ikoma",
    "Imada",
    "Imagawa",
    "Imai",
    "Imaizumi",
    "Imamura",
    "Imoo",
    "Ina",
    "Inaba",
    "Inao",
    "Inihara",
    "Ino",
    "Inoguchi",
    "Inokuma",
    "Inomata",
    "Inoue",
    "Inouye",
    "Inukai",
    "Ippitsusai",
    "Irie",
    "Iriye",
    "Isaka",
    "Isayama",
    "Ise",
    "Iseki",
    "Iseya",
    "Ishibashi",
    "Ishida",
    "Ishiguro",
    "Ishihara",
    "Ishikawa",
    "Ishimaru",
    "Ishimura",
    "Ishinomori",
    "Ishio",
    "Ishiyama",
    "Isobe",
    "Isoda",
    "Isozaki",
    "Itagaki",
    "Itami",
    "Ito",
    "Itoh",
    "Iwahara",
    "Iwahashi",
    "Iwakura",
    "Iwasa",
    "Iwasaki",
    "Izawa",
    "Izumi",
    // J Surnames
    "Jinnai",
    "Jo",
    "Joshuya",
    "Joshuyo",
    "Jukodo",
    "Jumonji",
    // K Surnames
    "Kada",
    "Kagabu",
    "Kagawa",
    "Kahae",
    "Kahaya",
    "Kai",
    "Kaibara",
    "Kaima",
    "Kajahara",
    "Kajitani",
    "Kajiwara",
    "Kajiyama",
    "Kakinomoto",
    "Kakutama",
    "Kamachi",
    "Kamata",
    "Kamei",
    "Kameyama",
    "Kaminaga",
    "Kamio",
    "Kamioka",
    "Kamisaka",
    "Kamo",
    "Kamon",
    "Kan",
    "Kanada",
    "Kanagaki",
    "Kanegawa",
    "Kaneko",
    "Kanesaka",
    "Kano",
    "Karamorita",
    "Karube",
    "Karubo",
    "Kasahara",
    "Kasai",
    "Kasamatsu",
    "Kasaya",
    "Kase",
    "Kashiwabara",
    "Kashiwagi",
    "Kasuse",
    "Katabuchi",
    "Kataoka",
    "Katayama",
    "Katayanagi",
    "Kate",
    "Kato",
    "Katoaka",
    "Katsu",
    "Katsukawa",
    "Katsumata",
    "Katsura",
    "Katsushika",
    "Kawabata",
    "Kawabe",
    "Kawachi",
    "Kawagichi",
    "Kawagishi",
    "Kawaguchi",
    "Kawai",
    "Kawaii",
    "Kawakami",
    "Kawamata",
    "Kawamura",
    "Kawano",
    "Kawasaki",
    "Kawasawa",
    "Kawashima",
    "Kawasie",
    "Kawatake",
    "Kawate",
    "Kawayama",
    "Kawazu",
    "Kaza",
    "Kazuyoshi",
    "Kenkyusha",
    "Kenmotsu",
    "Kentaro",
    "Ki",
    "Kido",
    "Kihara",
    "Kijimuta",
    "Kijmuta",
    "Kikkawa",
    "Kikuchi",
    "Kikugawa",
    "Kikui",
    "Kikutake",
    "Kimio",
    "Kimiyama",
    "Kimura",
    "Kinashita",
    "Kinjo",
    "Kino",
    "Kinoshita",
    "Kinugasa",
    "Kira",
    "Kishi",
    "Kiski",
    "Kita",
    "Kitabatake",
    "Kitagawa",
    "Kitamura",
    "Kitano",
    "Kitao",
    "Kitoaji",
    "Kiyoura",
    "Ko",
    "Kobayashi",
    "Kobi",
    "Kodama",
    "Koga",
    "Koganezawa",
    "Kogara",
    "Kogo",
    "Koguchi",
    "Koike",
    "Koiso",
    "Koizumi",
    "Kojima",
    "Kokan",
    "Komagata",
    "Komatsu",
    "Komatsuzaki",
    "Komine",
    "Komiya",
    "Komon",
    "Komukai",
    "Komura",
    "Kon",
    "Konae",
    "Konda",
    "Kondo",
    "Konishi",
    "Kono",
    "Konoe",
    "Kora",
    "Koruba",
    "Koshin",
    "Kotabe",
    "Kotara",
    "Kotoku",
    "Kouda",
    "Koyama",
    "Koyanagi",
    "Kozu",
    "Kubo",
    "Kubodera",
    "Kubota",
    "Kudara",
    "Kudo",
    "Kuga",
    "Kumagae",
    "Kumagai",
    "Kumasaka",
    "Kunda",
    "Kunikida",
    "Kunisada",
    "Kuno",
    "Kunomasu",
    "Kuramochi",
    "Kuramoto",
    "Kurata",
    "Kurkawa",
    "Kurmochi",
    "Kuroda",
    "Kurofuji",
    "Kurogane",
    "Kurohiko",
    "Kuroki",
    "Kurosawa",
    "Kurotani",
    "Kurusu",
    "Kusaka",
    "Kusatsu",
    "Kusonoki",
    "Kusuhara",
    "Kusumoto",
    "Kusunoki",
    "Kutsuna",
    "Kuwabara",
    "Kyubei",
    // M Surnames
    "Maeda",
    "Maeno",
    "Maita",
    "Makioka",
    "Makuda",
    "Marubeni",
    "Marugo",
    "Maruyama",
    "Masanobu",
    "Masaoka",
    "Mashita",
    "Masuda",
    "Masuko",
    "Masuno",
    "Masuo",
    "Masuzoe",
    "Matano",
    "Matsubara",
    "Matsuda",
    "Matsukata",
    "Matsuki",
    "Matsumara",
    "Matsumiya",
    "Matsumoto",
    "Matsuo",
    "Matsuoka",
    "Matsura",
    "Matsushima",
    "Matsushina",
    "Matsushita",
    "Matsuzawa",
    "Mayuzumi",
    "Mazaki",
    "Mazawa",
    "Mihashi",
    "Miki",
    "Mimasuya",
    "Minabuchi",
    "Minatoya",
    "Minobe",
    "Misawa",
    "Mishima",
    "Mitsubishi",
    "Mitsukuri",
    "Mitsuwa",
    "Mitsuya",
    "Mitzusaka",
    "Miura",
    "Miyagi",
    "Miyahara",
    "Miyajima",
    "Miyake",
    "Miyamoto",
    "Miyata",
    "Miyazaki",
    "Miyazawa",
    "Miyoshi",
    "Mizoguchi",
    "Mizukawa",
    "Mizukuro",
    "Mizuno",
    "Mizutani",
    "Mochizuki",
    "Modegi",
    "Momotami",
    "Momotani",
    "Mori",
    "Moriguchi",
    "Morimoto",
    "Morinaga",
    "Morioka",
    "Morita",
    "Moriwaka",
    "Morri",
    "Moto",
    "Motoori",
    "Munkata",
    "Muraguchi",
    "Murakami",
    "Muraoka",
    "Murata",
    "Murkami",
    "Muro",
    "Muruyama",
    "Muso",
    "Mutsu",
    // N Surnames
    "Nagahama",
    "Nagai",
    "Nagako",
    "Nagano",
    "Nagasawa",
    "Nagase",
    "Nagashima",
    "Nagata",
    "Nagatsuka",
    "Nagumo",
    "Naito",
    "Nakada",
    "Nakadai",
    "Nakadan",
    "Nakae",
    "Nakagawa",
    "Nakahara",
    "Nakajima",
    "Nakamoto",
    "Nakamura",
    "Nakane",
    "Nakanishi",
    "Nakano",
    "Nakanoi",
    "Nakao",
    "Nakasato",
    "Nakasawa",
    "Nakasone",
    "Nakata",
    "Nakatoni",
    "Nakatsuka",
    "Nakayama",
    "Nakazawa",
    "Namiki",
    "Nanami",
    "Narahashi",
    "Narato",
    "Narita",
    "Nataga",
    "Natsume",
    "Nawabe",
    "Nemoto",
    "Niijima",
    "Nijo",
    "Ninomiya",
    "Nishi",
    "Nishihara",
    "Nishikawa",
    "Nishimoto",
    "Nishimura",
    "Nishimuraya",
    "Nishio",
    "Nishiwaki",
    "Nishiyama",
    "Nitta",
    "Nobunaga",
    "Nobusawa",
    "Noda",
    "Nogi",
    "Noguchi",
    "Nogushi",
    "Nomura",
    "Nonomura",
    "Noro",
    "Nosaka",
    "Nose",
    "Noto",
    "Nozaki",
    "Nozara",
    "Numajiri",
    "Numata",
    // O Surnames
    "Obata",
    "Obinata",
    "Obuchi",
    "Ochi",
    "Ochiai",
    "Ochida",
    "Odaka",
    "Ogata",
    "Ogawa",
    "Ogiwara",
    "Ogura",
    "Ogyu",
    "Ohba",
    "Ohira",
    "Ohishi",
    "Ohka",
    "Ohmae",
    "Ohmiya",
    "Oichi",
    "Oinuma",
    "Oishi",
    "Okabe",
    "Okada",
    "Okajima",
    "Okakura",
    "Okamoto",
    "Okamura",
    "Okanao",
    "Okanaya",
    "Okano",
    "Okasawa",
    "Okawa",
    "Okazaki",
    "Okazawaya",
    "Okimasa",
    "Okimoto",
    "Okimura",
    "Okita",
    "Okubo",
    "Okuda",
    "Okui",
    "Okuma",
    "Okumura",
    "Okura",
    "Omori",
    "Omura",
    "Onishi",
    "Ono",
    "Onoda",
    "Onoe",
    "Onohara",
    "Ooka",
    "Oonishi",
    "Osagawa",
    "Osaka",
    "Osaragi",
    "Oshima",
    "Oshin",
    "Oshiro",
    "Ota",
    "Otaka",
    "Otake",
    "Otani",
    "Otomo",
    "Otsu",
    "Otsuka",
    "Ouchi",
    "Oushima",
    "Outakara",
    "Outsuka",
    "Oyama",
    "Ozaki",
    "Ozawa",
    "Ozu",
    // R Surnames
    "Raikatuji",
    "Royama",
    "Ryusaki",
    // S Surnames
    "Sada",
    "Saeki",
    "Saga",
    "Sahashi",
    "Saigo",
    "Saiki",
    "Saionji",
    "Saito",
    "Saitoh",
    "Saji",
    "Sakagami",
    "Sakai",
    "Sakakibara",
    "Sakamoto",
    "Sakanoue",
    "Sakata",
    "Sakiyurai",
    "Sako",
    "Sakoda",
    "Sakubara",
    "Sakuma",
    "Sakuraba",
    "Sakurada",
    "Sakurai",
    "Sammiya",
    "Sanda",
    "Sanjo",
    "Sano",
    "Santo",
    "Saromi",
    "Sarumara",
    "Sasada",
    "Sasakawa",
    "Sasaki",
    "Sassa",
    "Satake",
    "Sato",
    "Satoh",
    "Satou",
    "Satoya",
    "Sawai",
    "Sawamatsu",
    "Sawamura",
    "Sayuki",
    "Segawa",
    "Sekigawa",
    "Sekine",
    "Sekozawa",
    "Sen",
    "Senmatsu",
    "Seo",
    "Serizawa",
    "Seyama",
    "Shiba",
    "Shibaguchi",
    "Shibanuma",
    "Shibasaki",
    "Shibasawa",
    "Shibata",
    "Shibue",
    "Shibukji",
    "Shichirobei",
    "Shidehara",
    "Shiga",
    "Shiganori",
    "Shige",
    "Shigeki",
    "Shigemitsu",
    "Shigi",
    "Shikitei",
    "Shikuk",
    "Shima",
    "Shimada",
    "Shimakage",
    "Shimamura",
    "Shimanouchi",
    "Shimaoka",
    "Shimazaki",
    "Shimazu",
    "Shimedzu",
    "Shimizu",
    "Shimohira",
    "Shimon",
    "Shimura",
    "Shimuzu",
    "Shinko",
    "Shinozaki",
    "Shinozuka",
    "Shintaro",
    "Shiokawa",
    "Shiomi",
    "Shiomiya",
    "Shionoya",
    "Shiotani",
    "Shioya",
    "Shirahata",
    "Shirai",
    "Shiraishi",
    "Shirakawa",
    "Shirane",
    "Shirasu",
    "Shiratori",
    "Shirokawa",
    "Shiroyama",
    "Shiskikura",
    "Shizuma",
    "Shobo",
    "Shoda",
    "Shunji",
    "Shunsen",
    "Siagyo",
    "Soga",
    "Sohda",
    "Soho",
    "Soma",
    "Someya",
    "Sone",
    "Sonoda",
    "Soseki",
    "Sotomura",
    "Suenami",
    "Sugai",
    "Sugase",
    "Sugawara",
    "Sugihara",
    "Sugimoto",
    "Sugimura",
    "Sugino",
    "Sugisata",
    "Sugita",
    "Sugitani",
    "Sugiyama",
    "Sumitimo",
    "Sunada",
    "Suzambo",
    "Suzuki",
    // T Surnames
    "Tabuchi",
    "Tadeshi",
    "Tagawa",
    "Taguchi",
    "Taira",
    "Taka",
    "Takabe",
    "Takagaki",
    "Takagawa",
    "Takagi",
    "Takahama",
    "Takahashi",
    "Takahasi",
    "Takaki",
    "Takamura",
    "Takano",
    "Takaoka",
    "Takara",
    "Takashita",
    "Takasu",
    "Takasugi",
    "Takayama",
    "Takecare",
    "Takei",
    "Takekawa",
    "Takemago",
    "Takemitsu",
    "Takemura",
    "Takeshi",
    "Takeshita",
    "Taketomo",
    "Takeuchi",
    "Takeushi",
    "Takewaki",
    "Takimoto",
    "Takishida",
    "Takishita",
    "Takita",
    "Takizawa",
    "Taku",
    "Takudo",
    "Takudome",
    "Tamaasa",
    "Tamazaki",
    "Tamura",
    "Tamuro",
    "Tanaka",
    "Tange",
    "Tani",
    "Taniguchi",
    "Tanizaki",
    "Tankoshitsu",
    "Tansho",
    "Tanuma",
    "Tarumi",
    "Tatenaka",
    "Tateno",
    "Tatsuko",
    "Tatsuno",
    "Tatsuya",
    "Tawaraya",
    "Tayama",
    "Temko",
    "Tenshin",
    "Terada",
    "Terajima",
    "Terakado",
    "Terauchi",
    "Teshigahara",
    "Teshima",
    "Tezuka",
    "Tochikura",
    "Toda",
    "Togo",
    "Tojo",
    "Tokaji",
    "Tokuda",
    "Tokudome",
    "Tokuoka",
    "Tomika",
    "Tomimoto",
    "Tomioka",
    "Tommii",
    "Tomonaga",
    "Tomori",
    "Tono",
    "Torii",
    "Torisawa",
    "Torisei",
    "Toru",
    "Toshishai",
    "Toshitala",
    "Toshusai",
    "Toyama",
    "Toyoda",
    "Toyoshima",
    "Toyota",
    "Toyotomi",
    "Tsubouchi",
    "Tsucgimoto",
    "Tsuchie",
    "Tsuchiyama",
    "Tsuda",
    "Tsuga",
    "Tsuji",
    "Tsujimoto",
    "Tsujimura",
    "Tsukada",
    "Tsukade",
    "Tsukahara",
    "Tsukamoto",
    "Tsukatani",
    "Tsukawaki",
    "Tsukehara",
    "Tsukioka",
    "Tsumemasa",
    "Tsumura",
    "Tsunoda",
    "Tsurimi",
    "Tsuruga",
    "Tsuruya",
    "Tsushima",
    "Tsutaya",
    "Tsutomu",
    "Tsutsumi",
    "Tsutsumida",
    // U Surnames
    "Uboshita",
    "Uchida",
    "Uchiyama",
    "Ueda",
    "Uehara",
    "Uemura",
    "Ueshima",
    "Uesugi",
    "Uetake",
    "Ugaki",
    "Ui",
    "Ukiyo",
    "Umari",
    "Umehara",
    "Umeki",
    "Uno",
    "Uoya",
    "Urayama",
    "Urogataya",
    "Usami",
    "Ushiba",
    "Utagawa",
    // W Surnames
    "Wakai",
    "Wakatsuki",
    "Watabe",
    "Watanabe",
    "Watari",
    "Watnabe",
    "Watoga",
    // Y Surnames
    "Yakuta",
    "Yamabe",
    "Yamada",
    "Yamadera",
    "Yamagata",
    "Yamaguchi",
    "Yamaguchiya",
    "Yamaha",
    "Yamahata",
    "Yamakage",
    "Yamakawa",
    "Yamakazi",
    "Yamamoto",
    "Yamamura",
    "Yamana",
    "Yamanaka",
    "Yamane",
    "Yamanouchi",
    "Yamanoue",
    "Yamaoka",
    "Yamasaki",
    "Yamashita",
    "Yamato",
    "Yamauchi",
    "Yamawaki",
    "Yamazaki",
    "Yamhata",
    "Yamura",
    "Yanagawa",
    "Yanagi",
    "Yanagimoto",
    "Yanagita",
    "Yanasaki",
    "Yano",
    "Yasuda",
    "Yasuhiro",
    "Yasui",
    "Yasujiro",
    "Yasukawa",
    "Yasutake",
    "Yoemon",
    "Yokokawa",
    "Yokoyama",
    "Yonai",
    "Yone",
    "Yosano",
    "Yoshida",
    "Yoshida",
    "Yoshifumi",
    "Yoshihara",
    "Yoshikawa",
    "Yoshimatsu",
    "Yoshinobu",
    "Yoshioka",
    "Yoshitomi",
    "Yoshizaki",
    "Yoshizawa",
    "Yuasa",
    "Yuhara",
    "Yunokawa",
];
