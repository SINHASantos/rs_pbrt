// std
use std::cell::Cell;
use std::sync::Arc;
// pbrt
// use crate::core::bssrdf::Bssrdf;
use crate::core::camera::Camera;
use crate::core::geometry::{vec3_abs_dot_nrmf, vec3_dot_nrmf};
use crate::core::geometry::{Bounds2i, Point2f, Ray, Vector3f};
use crate::core::integrator::uniform_sample_one_light;
use crate::core::interaction::{Interaction, SurfaceInteraction};
use crate::core::lightdistrib::create_light_sample_distribution;
use crate::core::lightdistrib::LightDistribution;
use crate::core::material::TransportMode;
use crate::core::pbrt::{Float, Spectrum};
use crate::core::reflection::BxdfType;
use crate::core::sampler::Sampler;
use crate::core::sampling::Distribution1D;
use crate::core::scene::Scene;

// see path.h

/// Path Tracing (Global Illumination) - uses the render loop of a
/// [SamplerIntegrator](../../core/integrator/enum.SamplerIntegrator.html)
pub struct PathIntegrator {
    // inherited from SamplerIntegrator (see integrator.h)
    pub camera: Arc<Camera>,
    pub sampler: Box<Sampler>,
    pixel_bounds: Bounds2i,
    // see path.h
    max_depth: u32,
    rr_threshold: Float,           // 1.0
    light_sample_strategy: String, // "spatial"
    light_distribution: Option<Arc<LightDistribution>>,
}

impl PathIntegrator {
    pub fn new(
        max_depth: u32,
        camera: Arc<Camera>,
        sampler: Box<Sampler>,
        pixel_bounds: Bounds2i,
        rr_threshold: Float,
        light_sample_strategy: String,
    ) -> Self {
        PathIntegrator {
            camera,
            sampler,
            pixel_bounds,
            max_depth,
            rr_threshold,
            light_sample_strategy,
            light_distribution: None,
        }
    }
    pub fn preprocess(&mut self, scene: &Scene) {
        self.light_distribution =
            create_light_sample_distribution(self.light_sample_strategy.clone(), scene);
    }
    pub fn li(
        &self,
        r: &Ray,
        scene: &Scene,
        sampler: &mut Sampler,
        // arena: &mut Arena,
        _depth: i32,
    ) -> Spectrum {
        // TODO: ProfilePhase p(Prof::SamplerIntegratorLi);
        let mut l: Spectrum = Spectrum::default();
        let mut beta: Spectrum = Spectrum::new(1.0 as Float);
        let mut ray: Ray = Ray {
            o: r.o,
            d: r.d,
            t_max: Cell::new(r.t_max.get()),
            time: r.time,
            differential: r.differential,
            medium: r.medium.clone(),
        };
        let mut specular_bounce: bool = false;
        let mut bounces: u32 = 0_u32;
        // Added after book publication: etaScale tracks the
        // accumulated effect of radiance scaling due to rays passing
        // through refractive boundaries (see the derivation on p. 527
        // of the third edition). We track this value in order to
        // remove it from beta when we apply Russian roulette; this is
        // worthwhile, since it lets us sometimes avoid terminating
        // refracted rays that are about to be refracted back out of a
        // medium and thus have their beta value increased.
        let mut eta_scale: Float = 1.0;
        loop {
            // find next path vertex and accumulate contribution
            // println!("Path tracer bounce {:?}, current L = {:?}, beta = {:?}",
            //          bounces, l, beta);
            // intersect _ray_ with scene and store intersection in _isect_
            let mut isect: SurfaceInteraction = SurfaceInteraction::default();
            if scene.intersect(&ray, &mut isect) {
                // possibly add emitted light at intersection
                if bounces == 0 || specular_bounce {
                    // add emitted light at path vertex
                    l += beta * isect.le(&-ray.d);
                    // println!("Added Le -> L = {:?}", l);
                }
                // terminate path if _maxDepth_ was reached
                if bounces >= self.max_depth {
                    break;
                }
                // compute scattering functions and skip over medium boundaries
                let mode: TransportMode = TransportMode::Radiance;
                isect.compute_scattering_functions(&ray, true, mode);
                if let Some(ref _bsdf) = isect.bsdf {
                    // we are fine (for below)
                } else {
                    // TODO: println!("Skipping intersection due to null bsdf");
                    ray = isect.spawn_ray(&ray.d);
                    // bounces--;
                    continue;
                }
                if let Some(ref light_distribution) = self.light_distribution {
                    let distrib: Arc<Distribution1D> = light_distribution.lookup(&isect.common.p);
                    // Sample illumination from lights to find path contribution.
                    // (But skip this for perfectly specular BSDFs.)
                    let bsdf_flags: u8 = BxdfType::BsdfAll as u8 & !(BxdfType::BsdfSpecular as u8);
                    if let Some(ref bsdf) = isect.bsdf {
                        if bsdf.num_components(bsdf_flags) > 0 {
                            // TODO: ++total_paths;
                            let it: &SurfaceInteraction = &isect;
                            let ld: Spectrum = beta
                                * uniform_sample_one_light(
                                    it,
                                    scene,
                                    sampler,
                                    false,
                                    Some(&distrib),
                                );
                            // TODO: println!("Sampled direct lighting Ld = {:?}", ld);
                            // TODO: if ld.is_black() {
                            //     ++zero_radiance_paths;
                            // }
                            assert!(ld.y() >= 0.0 as Float, "ld = {:?}", ld);
                            l += ld;
                        }
                        // Sample BSDF to get new path direction
                        let wo: Vector3f = -ray.d;
                        let mut wi: Vector3f = Vector3f::default();
                        let mut pdf: Float = 0.0 as Float;
                        let bsdf_flags: u8 = BxdfType::BsdfAll as u8;
                        let mut sampled_type: u8 = u8::max_value(); // != 0
                        let f: Spectrum = bsdf.sample_f(
                            &wo,
                            &mut wi,
                            &sampler.get_2d(),
                            &mut pdf,
                            bsdf_flags,
                            &mut sampled_type,
                        );

                        // println!("Sampled BSDF, f = {:?}, pdf = {:?}", f, pdf);
                        if f.is_black() || pdf == 0.0 as Float {
                            break;
                        }
                        beta *= (f * vec3_abs_dot_nrmf(&wi, &isect.shading.n)) / pdf;
                        // println!("Updated beta = {:?}", beta);
                        assert!(beta.y() >= 0.0 as Float);
                        assert!(
                            !(beta.y().is_infinite()),
                            "[{:#?}, {:?}] = ({:#?} * dot({:#?}, {:#?})) / {:?}",
                            sampler.get_current_pixel(),
                            sampler.get_current_sample_number(),
                            f,
                            wi,
                            isect.shading.n,
                            pdf
                        );
                        specular_bounce = (sampled_type & BxdfType::BsdfSpecular as u8) != 0_u8;
                        if ((sampled_type & BxdfType::BsdfSpecular as u8) != 0_u8)
                            && ((sampled_type & BxdfType::BsdfTransmission as u8) != 0_u8)
                        {
                            let eta: Float = bsdf.eta;
                            // Update the term that tracks radiance
                            // scaling for refraction depending on
                            // whether the ray is entering or leaving
                            // the medium.
                            if vec3_dot_nrmf(&wo, &isect.common.n) > 0.0 as Float {
                                eta_scale *= eta * eta;
                            } else {
                                eta_scale *= 1.0 as Float / (eta * eta);
                            }
                        }
                        ray = isect.spawn_ray(&wi);

                        // account for subsurface scattering, if applicable
                        if let Some(ref bssrdf) = isect.bssrdf {
                            if (sampled_type & BxdfType::BsdfTransmission as u8) != 0_u8 {
                                // importance sample the BSSRDF
                                let s2: Point2f = sampler.get_2d();
                                let s1: Float = sampler.get_1d();
                                let (s, pi_opt) = bssrdf.sample_s(
                                    // the next three (extra) parameters are used for SeparableBssrdfAdapter
                                    bssrdf.clone(),
                                    bssrdf.mode,
                                    bssrdf.eta,
                                    // done
                                    scene,
                                    s1,
                                    s2,
                                    &mut pdf,
                                );
                                if s.is_black() || pdf == 0.0 as Float {
                                    break;
                                }
                                assert!(!(beta.y().is_infinite()));
                                beta *= s / pdf;
                                if let Some(pi) = pi_opt {
                                    // account for the direct subsurface scattering component
                                    let distrib: Arc<Distribution1D> =
                                        light_distribution.lookup(&pi.common.p);
                                    l += beta
                                        * uniform_sample_one_light(
                                            &pi,
                                            scene,
                                            sampler,
                                            false,
                                            Some(&distrib),
                                        );
                                    // account for the indirect subsurface scattering component
                                    let mut wi: Vector3f = Vector3f::default();
                                    let mut pdf: Float = 0.0 as Float;
                                    let bsdf_flags: u8 = BxdfType::BsdfAll as u8;
                                    let mut sampled_type: u8 = u8::max_value(); // != 0
                                    if let Some(ref bsdf) = pi.bsdf {
                                        let f: Spectrum = bsdf.sample_f(
                                            &pi.common.wo,
                                            &mut wi,
                                            &sampler.get_2d(),
                                            &mut pdf,
                                            bsdf_flags,
                                            &mut sampled_type,
                                        );
                                        if f.is_black() || pdf == 0.0 as Float {
                                            break;
                                        }
                                        beta *= f * vec3_abs_dot_nrmf(&wi, &pi.shading.n) / pdf;
                                        assert!(!(beta.y().is_infinite()));
                                        specular_bounce =
                                            (sampled_type & BxdfType::BsdfSpecular as u8) != 0_u8;
                                        ray = pi.spawn_ray(&wi);
                                    }
                                }
                            }
                        }

                        // Possibly terminate the path with Russian roulette.
                        // Factor out radiance scaling due to refraction in rr_beta.
                        let rr_beta: Spectrum = beta * eta_scale;
                        if rr_beta.max_component_value() < self.rr_threshold && bounces > 3 {
                            let q: Float =
                                (0.05 as Float).max(1.0 as Float - rr_beta.max_component_value());
                            if sampler.get_1d() < q {
                                break;
                            }
                            beta /= 1.0 as Float - q;
                            assert!(!(beta.y().is_infinite()));
                        }
                    } else {
                        println!("TODO: if let Some(ref bsdf) = isect.bsdf failed");
                    }
                }
            } else {
                // add emitted light from the environment
                if bounces == 0 || specular_bounce {
                    // for (const auto &light : scene.infiniteLights)
                    for light in &scene.infinite_lights {
                        l += beta * light.le(&ray);
                    }
                    // println!("Added infinite area lights -> L = {:?}", l);
                }
                // terminate path if ray escaped
                break;
            }
            bounces += 1_u32;
        }
        l
    }
    pub fn get_camera(&self) -> Arc<Camera> {
        self.camera.clone()
    }
    pub fn get_sampler(&self) -> &Sampler {
        &self.sampler
    }
    pub fn get_pixel_bounds(&self) -> Bounds2i {
        self.pixel_bounds
    }
}
