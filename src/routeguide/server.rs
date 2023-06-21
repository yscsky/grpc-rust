use std::{
    cmp::{max, min},
    collections::HashMap,
    error::Error,
    hash::Hash,
    pin::Pin,
    sync::Arc,
    time::Instant,
};

use futures_core::Stream;
use futures_util::StreamExt;
use routeguide::{
    route_guide_server::{RouteGuide, RouteGuideServer},
    Feature, Point, Rectangle, RouteNote, RouteSummary,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

pub mod routeguide {
    tonic::include_proto!("routeguide");
}

mod data;

#[derive(Debug)]
struct RouteGuideService {
    features: Arc<Vec<Feature>>,
}

#[tonic::async_trait]
impl RouteGuide for RouteGuideService {
    async fn get_feature(&self, request: Request<Point>) -> Result<Response<Feature>, Status> {
        println!("GetFeature = {:?}", request);
        for feature in &self.features[..] {
            if feature.location.as_ref() == Some(request.get_ref()) {
                return Ok(Response::new(feature.clone()));
            }
        }
        Ok(Response::new(Feature::default()))
    }

    type ListFeaturesStream = ReceiverStream<Result<Feature, Status>>;

    async fn list_features(
        &self,
        request: Request<Rectangle>,
    ) -> Result<Response<Self::ListFeaturesStream>, Status> {
        println!("ListFeatures = {:?}", request);
        let (tx, rx) = mpsc::channel(4);
        let features = self.features.clone();
        tokio::spawn(async move {
            for feature in &features[..] {
                if in_range(feature.location.as_ref().unwrap(), request.get_ref()) {
                    println!("  => send {:?}", feature);
                    tx.send(Ok(feature.clone())).await.unwrap();
                }
            }
            println!(" /// done sending");
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn record_route(
        &self,
        request: Request<Streaming<Point>>,
    ) -> Result<Response<RouteSummary>, Status> {
        println!("RecordRoute");
        let mut stream = request.into_inner();
        let mut summary = RouteSummary::default();
        let mut last_point = None;
        let now = Instant::now();
        while let Some(point) = stream.next().await {
            let point = point?;
            println!("  ==> Point = {:?}", point);
            summary.point_count += 1;
            for feature in &self.features[..] {
                if feature.location.as_ref() == Some(&point) {
                    summary.feature_count += 1;
                }
            }
            if let Some(ref last_point) = last_point {
                summary.distance += calc_distance(last_point, &point);
            }
            last_point = Some(point);
        }
        summary.elapsed_time = now.elapsed().as_secs() as i32;
        Ok(Response::new(summary))
    }

    type RouteChatStream = Pin<Box<dyn Stream<Item = Result<RouteNote, Status>> + Send + 'static>>;

    async fn route_chat(
        &self,
        request: Request<Streaming<RouteNote>>,
    ) -> Result<Response<Self::RouteChatStream>, Status> {
        let mut notes = HashMap::new();
        let mut stream = request.into_inner();
        let output = async_stream::try_stream! {
            while let Some(note) = stream.next().await {
                let note = note?;
                let location =  note.location.clone().unwrap();
                let location_notes  = notes.entry(location).or_insert(vec![]);
                location_notes.push(note);
                for note in location_notes {
                    yield note.clone();
                }
            }
        };
        Ok(Response::new(Box::pin(output) as Self::RouteChatStream))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "[::1]:10000".parse().unwrap();
    println!("RouteGuideServer listening on: {}", addr);
    let route_guide = RouteGuideService {
        features: Arc::new(data::load()),
    };
    let svc = RouteGuideServer::new(route_guide);
    Server::builder().add_service(svc).serve(addr).await?;
    Ok(())
}

impl Eq for Point {}

impl Hash for Point {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.latitude.hash(state);
        self.longitude.hash(state);
    }
}

fn in_range(point: &Point, rect: &Rectangle) -> bool {
    let lo = rect.lo.as_ref().unwrap();
    let hi = rect.hi.as_ref().unwrap();

    let left = min(lo.longitude, hi.longitude);
    let right = max(lo.longitude, hi.longitude);
    let top = max(lo.latitude, hi.latitude);
    let bottom = min(lo.latitude, hi.latitude);

    point.longitude >= left
        && point.longitude <= right
        && point.latitude >= bottom
        && point.latitude <= top
}

fn calc_distance(p1: &Point, p2: &Point) -> i32 {
    const CORD_FACTOR: f64 = 1e7;
    const R: f64 = 6_371_000.0;

    let lat1 = p1.latitude as f64 / CORD_FACTOR;
    let lat2 = p2.latitude as f64 / CORD_FACTOR;
    let lng1 = p1.longitude as f64 / CORD_FACTOR;
    let lng2 = p2.longitude as f64 / CORD_FACTOR;

    let lat_rad1 = lat1.to_radians();
    let lat_rad2 = lat2.to_radians();

    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lng = (lng2 - lng1).to_radians();

    let a = (delta_lat / 2f64).sin() * (delta_lat / 2f64).sin()
        + lat_rad1.cos() * lat_rad2.cos() * (delta_lng / 2f64).sin() * (delta_lng / 2f64).sin();
    let c = 2f64 * a.sqrt().atan2((1f64 - a).sqrt());
    (R * c) as i32
}
