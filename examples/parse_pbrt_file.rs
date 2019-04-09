extern crate getopts;
extern crate pbrt;
// pest
extern crate pest;
#[macro_use]
extern crate pest_derive;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[grammar = "../examples/parse_pbrt.pest"]
struct PbrtParser;

// parser
use pest::Parser;

// getopts
use getopts::Options;
// pbrt
use pbrt::core::api::{
    pbrt_attribute_begin, pbrt_attribute_end, pbrt_camera, pbrt_cleanup, pbrt_film, pbrt_init,
    pbrt_integrator, pbrt_light_source, pbrt_look_at, pbrt_material, pbrt_rotate, pbrt_sampler,
    pbrt_shape, pbrt_translate, pbrt_world_begin,
};
use pbrt::core::api::{ApiState, BsdfState};
use pbrt::core::geometry::{Normal3f, Point2f, Point3f, Vector3f};
use pbrt::core::paramset::ParamSet;
use pbrt::core::pbrt::{Float, Spectrum};
// std
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn print_version(program: &str) {
    println!("{} {}", program, VERSION);
}

// ActiveTransform
// AreaLightSource
// Accelerator
// ConcatTransform
// CoordinateSystem
// CoordSysTransform
// Include
// Identity
// MakeNamedMaterial
// MakeNamedMedium
// MediumInterface
// NamedMaterial
// ObjectBegin
// ObjectEnd
// ObjectInstance
// PixelFilter
// ReverseOrientation
// Rotate
// Scale
// TransformBegin
// TransformEnd
// Transform
// Translate
// TransformTimes
// Texture

fn pbrt_bool_parameter(pairs: &mut pest::iterators::Pairs<Rule>) -> (String, bool) {
    // single string with or without brackets
    let ident = pairs.next();
    let string: String = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    let option = pairs.next();
    let lbrack = option.clone().unwrap();
    let string2: String;
    if lbrack.as_str() == "[" {
        // check for brackets
        let string = pairs.next();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    } else {
        // no brackets
        let string = option.clone();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    }
    // return boolean (instead of string)
    let b: bool;
    if string2 == "true" {
        b = true;
    } else if string2 == "false" {
        b = false
    } else {
        println!(
            "WARNING: parameter {:?} not well defined, defaulting to false",
            string
        );
        b = false
    }
    (string, b)
}

fn pbrt_float_parameter(pairs: &mut pest::iterators::Pairs<Rule>) -> (String, Vec<Float>) {
    let mut floats: Vec<Float> = Vec::new();
    // single float or several floats using brackets
    let ident = pairs.next();
    let string: String = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    let option = pairs.next();
    let lbrack = option.clone().unwrap();
    if lbrack.as_str() == "[" {
        // check for brackets
        let mut number = pairs.next();
        while number.is_some() {
            let pair = number.unwrap().clone();
            if pair.as_str() == "]" {
                // closing bracket found
                break;
            } else {
                let float: Float = f32::from_str(pair.into_span().as_str()).unwrap();
                floats.push(float);
            }
            number = pairs.next();
        }
    } else {
        // no brackets
        let mut number = option.clone();
        while number.is_some() {
            let pair = number.unwrap().clone();
            let float: Float = f32::from_str(pair.into_span().as_str()).unwrap();
            floats.push(float);
            number = pairs.next();
        }
    }
    (string, floats)
}

fn pbrt_integer_parameter(pairs: &mut pest::iterators::Pairs<Rule>) -> (String, Vec<i32>) {
    let mut integers: Vec<i32> = Vec::new();
    // single integer or several integers using brackets
    let ident = pairs.next();
    let string: String = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    let option = pairs.next();
    let lbrack = option.clone().unwrap();
    if lbrack.as_str() == "[" {
        // check for brackets
        let mut number = pairs.next();
        while number.is_some() {
            let pair = number.unwrap().clone();
            if pair.as_str() == "]" {
                // closing bracket found
                break;
            } else {
                let integer: i32 = i32::from_str(pair.into_span().as_str()).unwrap();
                integers.push(integer);
            }
            number = pairs.next();
        }
    } else {
        // no brackets
        let mut number = option.clone();
        while number.is_some() {
            let pair = number.unwrap().clone();
            let integer: i32 = i32::from_str(pair.into_span().as_str()).unwrap();
            integers.push(integer);
            number = pairs.next();
        }
    }
    (string, integers)
}

fn pbrt_string_parameter(pairs: &mut pest::iterators::Pairs<Rule>) -> (String, String) {
    // single string with or without brackets
    let ident = pairs.next();
    let string1: String = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    let option = pairs.next();
    let lbrack = option.clone().unwrap();
    let string2: String;
    if lbrack.as_str() == "[" {
        // check for brackets
        let string = pairs.next();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    } else {
        // no brackets
        let string = option.clone();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    }
    (string1, string2)
}

fn pbrt_texture_parameter(pairs: &mut pest::iterators::Pairs<Rule>) -> (String, String) {
    // single string with or without brackets
    let ident = pairs.next();
    let string1: String = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    let option = pairs.next();
    let lbrack = option.clone().unwrap();
    let string2: String;
    if lbrack.as_str() == "[" {
        // check for brackets
        let string = pairs.next();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    } else {
        // no brackets
        let string = option.clone();
        let pair = string.unwrap().clone();
        let ident = pair.into_inner().next();
        string2 = String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
    }
    (string1, string2)
}

fn extract_params(key_word: String, pairs: pest::iterators::Pair<Rule>) -> ParamSet {
    let mut params: ParamSet = ParamSet::default();
    params.key_word = key_word;
    let mut counter: u8 = 0_u8;
    for pair in pairs.into_inner() {
        // let span = pair.clone().into_span();
        // println!("Rule:    {:?}", pair.as_rule());
        // println!("Span:    {:?}", span);
        // println!("Text:    {}", span.as_str());
        match pair.as_rule() {
            Rule::string => {
                match counter {
                    0 => {
                        // name
                        let mut string_pairs = pair.into_inner();
                        let ident = string_pairs.next();
                        params.name =
                            String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
                    }
                    1 => {
                        // tex_type
                        let mut string_pairs = pair.into_inner();
                        let ident = string_pairs.next();
                        params.tex_type =
                            String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
                    }
                    2 => {
                        // tex_name
                        let mut string_pairs = pair.into_inner();
                        let ident = string_pairs.next();
                        params.tex_name =
                            String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
                    }
                    _ => unreachable!(),
                };
                counter += 1_u8;
            }
            Rule::type_name => {
                // name
                let mut string_pairs = pair.into_inner();
                let ident = string_pairs.next();
                params.name =
                    String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
            }
            Rule::file_name => {
                // name
                let mut string_pairs = pair.into_inner();
                let ident = string_pairs.next();
                params.name =
                    String::from_str(ident.unwrap().clone().into_span().as_str()).unwrap();
            }
            Rule::parameter => {
                for parameter_pair in pair.into_inner() {
                    match parameter_pair.as_rule() {
                        Rule::bool_param => {
                            let tuple: (String, bool) =
                                pbrt_bool_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let b: bool = tuple.1;
                            params.add_bool(string, b);
                        }
                        Rule::blackbody_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            params.add_blackbody_spectrum(string, floats);
                        }
                        Rule::float_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            if floats.len() == 1 {
                                params.add_float(string, floats[0]);
                            } else {
                                params.add_floats(string, floats);
                            }
                        }
                        Rule::integer_param => {
                            let tuple: (String, Vec<i32>) =
                                pbrt_integer_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let integers: Vec<i32> = tuple.1;
                            if integers.len() == 1 {
                                params.add_int(string, integers[0]);
                            } else {
                                params.add_ints(string, integers);
                            }
                        }
                        Rule::point_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            if floats.len() == 3 {
                                params.add_point3f(
                                    string,
                                    Point3f {
                                        x: floats[0],
                                        y: floats[1],
                                        z: floats[2],
                                    },
                                );
                            } else {
                                params.add_point3fs(string, floats);
                            }
                        }
                        Rule::point2_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            if floats.len() == 2 {
                                params.add_point2f(
                                    string,
                                    Point2f {
                                        x: floats[0],
                                        y: floats[1],
                                    },
                                );
                            } else {
                                params.add_point2fs(string, floats);
                            }
                        }
                        Rule::normal_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            if floats.len() == 3 {
                                params.add_normal3f(
                                    string,
                                    Normal3f {
                                        x: floats[0],
                                        y: floats[1],
                                        z: floats[2],
                                    },
                                );
                            } else {
                                params.add_normal3fs(string, floats);
                            }
                        }
                        Rule::rgb_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            params.add_rgb_spectrum(
                                string,
                                Spectrum {
                                    c: [floats[0], floats[1], floats[2]],
                                },
                            );
                        }
                        Rule::spectrum_param => {
                            // TODO: "spectrum Kd" [ 300 .3  400 .6   410 .65  415 .8  500 .2  600 .1 ]
                            // let tuple: (String, Vec<Float>) =
                            //     pbrt_float_parameter(&mut parameter_pair.into_inner());
                            // let string: String = tuple.0;
                            // let floats: Vec<Float> = tuple.1;
                            // params.add_rgb_spectrum(
                            //     string,
                            //     Spectrum {
                            //         c: [floats[0], floats[1], floats[2]],
                            //     },
                            // );
                            // or
                            // "spectrum Kd" "filename"
                            let tuple: (String, String) =
                                pbrt_string_parameter(&mut parameter_pair.into_inner());
                            let string1: String = tuple.0;
                            let string2: String = tuple.1;
                            let mut strings: Vec<String> = Vec::with_capacity(1_usize);
                            strings.push(string2);
                            params.add_sampled_spectrum_files(string1, strings);
                        }
                        Rule::string_param => {
                            let tuple: (String, String) =
                                pbrt_string_parameter(&mut parameter_pair.into_inner());
                            let string1: String = tuple.0;
                            let string2: String = tuple.1;
                            params.add_string(string1, string2);
                        }
                        Rule::texture_param => {
                            let tuple: (String, String) =
                                pbrt_texture_parameter(&mut parameter_pair.into_inner());
                            let string1: String = tuple.0;
                            let string2: String = tuple.1;
                            params.add_texture(string1, string2);
                        }
                        Rule::vector_param => {
                            let tuple: (String, Vec<Float>) =
                                pbrt_float_parameter(&mut parameter_pair.into_inner());
                            let string: String = tuple.0;
                            let floats: Vec<Float> = tuple.1;
                            if floats.len() == 3 {
                                params.add_vector3f(
                                    string,
                                    Vector3f {
                                        x: floats[0],
                                        y: floats[1],
                                        z: floats[2],
                                    },
                                );
                            } else {
                                params.add_vector3fs(string, floats);
                            }
                        }
                        // TODO: more rules
                        _ => println!("TODO: {:?}", parameter_pair.as_rule()),
                    }
                }
            }
            _ => println!("TODO: {:?}", pair.as_rule()),
        }
    }
    params
}

fn parse_line(
    api_state: &mut ApiState,
    bsdf_state: &mut BsdfState,
    identifier: &str,
    str_buf: String,
) {
    if str_buf == "" {
        // no additional arguments
        match identifier {
            "AttributeBegin" => {
                // AttributeBegin
                // println!("{} {}", identifier, str_buf);
                pbrt_attribute_begin(api_state);
            }
            "AttributeEnd" => {
                // AttributeEnd
                // println!("{} {}", identifier, str_buf);
                pbrt_attribute_end(api_state);
            }
            "WorldBegin" => {
                // WorldBegin
                // println!("{} {}", identifier, str_buf);
                pbrt_world_begin(api_state);
            }
            "WorldEnd" => {
                // WorldEnd
                // println!("{} {}", identifier, str_buf);
                pbrt_cleanup(api_state);
            }
            _ => println!("{} {}", identifier, str_buf),
        }
    } else {
        let pairs = PbrtParser::parse(Rule::name_and_or_params, &str_buf)
            .expect("unsuccessful parse")
            .next()
            .unwrap();
        for inner_pair in pairs.into_inner() {
            match inner_pair.as_rule() {
                Rule::type_params => {
                    // identifier "type" parameter-list
                    let for_printing = inner_pair.as_str();
                    let params = extract_params(String::from(identifier), inner_pair);
                    match identifier {
                        "Camera" => {
                            // Camera
                            pbrt_camera(api_state, params);
                        }
                        "Film" => {
                            // Film
                            pbrt_film(api_state, params);
                        }
                        "Integrator" => {
                            // Integrator
                            pbrt_integrator(api_state, params);
                        }
                        "LightSource" => {
                            // LightSource
                            pbrt_light_source(api_state, params);
                        }
                        "Material" => {
                            // Material
                            pbrt_material(api_state, params);
                        }
                        "Sampler" => {
                            // Sampler
                            pbrt_sampler(api_state, params);
                        }
                        "Shape" => {
                            // Shape
                            pbrt_shape(api_state, bsdf_state, params);
                        }
                        _ => println!("> {} {}", identifier, for_printing),
                    }
                }
                Rule::look_at => {
                    // LookAt eye_x eye_y eye_z look_x look_y look_z up_x up_y up_z
                    let mut v: Vec<Float> = Vec::new();
                    for rule_pair in inner_pair.into_inner() {
                        let number: Float =
                            f32::from_str(rule_pair.clone().into_span().as_str()).unwrap();
                        v.push(number);
                    }
                    // println!(
                    //     "LookAt {} {} {} {} {} {} {} {} {}",
                    //     v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8],
                    // );
                    pbrt_look_at(
                        api_state, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8],
                    );
                }
                Rule::rotate => {
                    // Rotate angle x y z
                    let mut v: Vec<Float> = Vec::new();
                    for rule_pair in inner_pair.into_inner() {
                        let number: Float =
                            f32::from_str(rule_pair.clone().into_span().as_str()).unwrap();
                        v.push(number);
                    }
                    // println!("Rotate {} {} {} {}", v[0], v[1], v[2], v[3]);
                    pbrt_rotate(api_state, v[0], v[1], v[2], v[3]);
                }
                Rule::translate => {
                    // Translate x y z
                    let mut v: Vec<Float> = Vec::new();
                    for rule_pair in inner_pair.into_inner() {
                        let number: Float =
                            f32::from_str(rule_pair.clone().into_span().as_str()).unwrap();
                        v.push(number);
                    }
                    // println!("Translate {} {} {}", v[0], v[1], v[2]);
                    pbrt_translate(api_state, v[0], v[1], v[2]);
                }
                Rule::remaining_line => {
                    // predetermined number of arguments of predetermined type
                    println!("< {} {}", identifier, inner_pair.as_str());
                }
                // _ => unreachable!(),
                _ => println!("TODO: {:?}", inner_pair.as_rule()),
            }
        }
    }
}

fn main() {
    // handle command line options
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("i", "", "parse an input file", "FILE");
    opts.optopt(
        "t",
        "nthreads",
        "use specified number of threads for rendering",
        "NUM",
    );
    opts.optflag("v", "version", "print version number");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    } else if matches.opt_present("i") {
        let mut number_of_threads: u8 = 0_u8;
        if matches.opt_present("t") {
            let nthreads = matches.opt_str("t");
            match nthreads {
                Some(x) => {
                    let number_result = x.parse::<u8>();
                    assert!(
                        !number_result.is_err(),
                        "ERROR: 8 bit unsigned integer expected"
                    );
                    let num_threads: u8 = number_result.unwrap();
                    println!("nthreads = {:?}", num_threads);
                    number_of_threads = num_threads;
                }
                None => panic!("No argument for number of threads given."),
            }
        }
        let infile = matches.opt_str("i");
        match infile {
            Some(x) => {
                let num_cores = num_cpus::get();
                println!("pbrt version {} [Detected {} cores]", VERSION, num_cores);
                println!("Copyright (c) 2016-2019 Jan Douglas Bert Walter.");
                println!(
                    "Rust code based on C++ code by Matt Pharr, Greg Humphreys, and Wenzel Jakob."
                );
                // println!("FILE = {}", x);
                let f = File::open(x.clone()).unwrap();
                let ip: &Path = Path::new(x.as_str());
                let (mut api_state, mut bsdf_state) = pbrt_init(number_of_threads);
                if ip.is_relative() {
                    let cp: PathBuf = env::current_dir().unwrap();
                    let pb: PathBuf = cp.join(ip);
                    let search_directory: &Path = pb.as_path().parent().unwrap();
                    // println!("search_directory is {}", search_directory.display());
                    api_state.search_directory = Some(Box::new(PathBuf::from(search_directory)));
                }
                let mut reader = BufReader::new(f);
                let mut str_buf: String = String::default();
                let _num_bytes = reader.read_to_string(&mut str_buf);
                // if num_bytes.is_ok() {
                //     let n_bytes = num_bytes.unwrap();
                //     println!("{} bytes read", n_bytes);
                // }
                let pairs = PbrtParser::parse(Rule::pbrt, &str_buf)
                    .expect("unsuccessful parse")
                    .next()
                    .unwrap();
                let mut identifier: &str = "";
                let mut comment_count: u64 = 0;
                let mut empty_count: u64 = 0;
                let mut todo_count: u64 = 0;
                let mut parse_again: String = String::default();
                // first parse file line by line
                for inner_pair in pairs.into_inner() {
                    match inner_pair.as_rule() {
                        // comment lines (starting with '#')
                        Rule::comment_line => {
                            comment_count += 1;
                        }
                        Rule::statement_line => {
                            for statement_pair in inner_pair.into_inner() {
                                match statement_pair.as_rule() {
                                    Rule::identifier => {
                                        if identifier != "" {
                                            parse_line(
                                                &mut api_state,
                                                &mut bsdf_state,
                                                identifier,
                                                parse_again.clone(),
                                            );
                                        }
                                        identifier = statement_pair.as_str();
                                        parse_again = String::default();
                                    }
                                    Rule::remaining_line => {
                                        if parse_again != "" {
                                            parse_again =
                                                parse_again + " " + statement_pair.as_str();
                                        } else {
                                            parse_again += statement_pair.as_str();
                                        }
                                    }
                                    Rule::trailing_comment => {
                                        // ignore
                                    }
                                    _ => println!("TODO: {:?}", statement_pair.as_rule()),
                                }
                            }
                        }
                        Rule::empty_line => {
                            empty_count += 1;
                        }
                        Rule::todo_line => {
                            todo_count += 1;
                            for params_pair in inner_pair.into_inner() {
                                match params_pair.as_rule() {
                                    Rule::remaining_params => {
                                        if parse_again != "" {
                                            parse_again = parse_again + " " + params_pair.as_str();
                                        } else {
                                            parse_again += params_pair.as_str();
                                        }
                                    }
                                    Rule::trailing_comment => {
                                        // ignore
                                    }
                                    _ => println!("TODO: {:?}", params_pair.as_rule()),
                                }
                            }
                        }
                        Rule::EOI => parse_line(
                            &mut api_state,
                            &mut bsdf_state,
                            identifier,
                            parse_again.clone(),
                        ),
                        _ => unreachable!(),
                    }
                }
                println!("Number of comment line(s):   {}", comment_count);
                println!("Number of parameter line(s): {}", todo_count);
                println!("Number of empty line(s):     {}", empty_count);
            }
            None => panic!("No input file name."),
        }
        return;
    } else if matches.opt_present("v") {
        print_version(&program);
        return;
    } else {
        print_usage(&program, opts);
        return;
    }
}
