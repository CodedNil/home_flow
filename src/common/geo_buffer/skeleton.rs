use std::cmp::Ordering;
use std::fmt;

use geo::winding_order::WindingOrder;
use geo::{Contains, Winding};
use geo_types::{LineString, MultiPolygon, Polygon};

use super::priority_queue::PriorityQueue;
use super::util::{feq, fgeq, fleq, fneq, Coordinate, Ray};
use super::vertex_queue::{IndexType, VertexQueue};

#[derive(Debug)]
pub enum VertexType {
    Tree {
        axis: Ray,
        left_ray: Ray,
        right_ray: Ray,
        parent: usize,
        time_elapsed: f64,
    },
    Split {
        location: Coordinate,
        time_elapsed: f64,
    },
    Root {
        location: Coordinate,
        time_elapsed: f64,
    },
}

impl VertexType {
    fn init_tree_vertex(lv: Coordinate, cv: Coordinate, rv: Coordinate, orient: bool) -> Self {
        let r1 = Ray::new(cv, lv);
        let r2 = Ray::new(cv, rv);
        let mut r3 = r1.bisector(&r2, cv, orient);
        r3.angle = r3.angle / (r3.point_by_ratio(1.).dist_ray(&r2));
        Self::Tree {
            axis: r3,
            left_ray: r1,
            right_ray: r2,
            parent: usize::MAX,
            time_elapsed: 0.,
        }
    }

    fn new_tree_vertex(location: Coordinate, left_ray: Ray, right_ray: Ray, orient: bool) -> Self {
        let mut axis = left_ray.bisector(&right_ray, location, orient);
        axis.angle = axis.angle
            / f64::abs(
                axis.point_by_ratio(1.).dist_ray(&left_ray)
                    - axis.point_by_ratio(0.).dist_ray(&left_ray),
            );
        let time_elapsed = axis.origin.dist_ray(&left_ray);
        Self::Tree {
            axis,
            left_ray,
            right_ray,
            parent: usize::MAX,
            time_elapsed,
        }
    }

    const fn new_split_vertex(location: Coordinate, time_elapsed: f64) -> Self {
        Self::Split {
            location,
            time_elapsed,
        }
    }

    const fn new_root_vertex(location: Coordinate, time_elapsed: f64) -> Self {
        Self::Root {
            location,
            time_elapsed,
        }
    }

    fn initialize_from_polygon_vector(
        input_polygon_vector: &Vec<Polygon>,
        orient: bool,
    ) -> Vec<Self> {
        let mut ret = Vec::new();
        for p in input_polygon_vector {
            let len = p.exterior().0.len() - 1;
            for cur in 0..len {
                let prv = (cur + len - 1) % len;
                let nxt = (cur + 1) % len;
                let new_vertex = Self::init_tree_vertex(
                    p.exterior().0[prv].into(),
                    p.exterior().0[cur].into(),
                    p.exterior().0[nxt].into(),
                    orient,
                );
                ret.push(new_vertex);
            }
            for i in 0..p.interiors().len() {
                let len = p.interiors()[i].0.len() - 1;
                for cur in 0..len {
                    let prv = (cur + len - 1) % len;
                    let nxt = (cur + 1) % len;
                    let new_node = Self::init_tree_vertex(
                        p.interiors()[i].0[prv].into(),
                        p.interiors()[i].0[cur].into(),
                        p.interiors()[i].0[nxt].into(),
                        orient,
                    );
                    ret.push(new_node);
                }
            }
        }
        ret
    }

    const fn unwrap_location(&self) -> Coordinate {
        match self {
            Self::Tree { axis, .. } => axis.origin,
            Self::Split { location, .. } | Self::Root { location, .. } => *location,
        }
    }

    const fn unwrap_time(&self) -> f64 {
        match self {
            Self::Tree { time_elapsed, .. }
            | Self::Split { time_elapsed, .. }
            | Self::Root { time_elapsed, .. } => *time_elapsed,
        }
    }

    fn unwrap_ray(&self) -> Ray {
        if let Self::Tree { axis, .. } = self {
            return *axis;
        }
        panic!("Expected VertexType::TreeVertex");
    }

    fn unwrap_base_ray(&self) -> (Ray, Ray) {
        if let Self::Tree {
            left_ray,
            right_ray,
            ..
        } = self
        {
            return (*left_ray, *right_ray);
        }
        panic!("Expected VertexType::TreeVertex but {self:?}");
    }

    fn set_parent(&mut self, nparent: usize) {
        if let Self::Tree { parent, .. } = self {
            *parent = nparent;
        } else {
            panic!("Expected VertexType::TreeVertex but {self:?}")
        };
    }
}

#[derive(PartialEq)]
enum Event {
    VertexEvent {
        time: f64,
        merge_from: usize,
        merge_to: usize,
    },
    EdgeEvent {
        time: f64,
        split_from: usize,
        split_into: usize,
        split_to_left: usize,
        split_to_right: usize,
    },
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let x1 = match self {
            Self::VertexEvent {
                time,
                merge_from,
                merge_to,
            } => (*time, *merge_from, *merge_to, 0, 0),
            Self::EdgeEvent {
                time,
                split_from,
                split_into,
                split_to_left,
                split_to_right,
            } => (
                *time,
                *split_from,
                *split_into,
                *split_to_left,
                *split_to_right,
            ),
        };
        let x2 = match other {
            Self::VertexEvent {
                time,
                merge_from,
                merge_to,
            } => (*time, *merge_from, *merge_to, 0, 0),
            Self::EdgeEvent {
                time,
                split_from,
                split_into,
                split_to_left,
                split_to_right,
            } => (
                *time,
                *split_from,
                *split_into,
                *split_to_left,
                *split_to_right,
            ),
        };
        Some(x1.partial_cmp(&x2).unwrap())
    }
}

impl Event {
    const fn unwrap_time(&self) -> f64 {
        match self {
            Self::VertexEvent { time, .. } | Self::EdgeEvent { time, .. } => *time,
        }
    }
}

#[derive(PartialEq)]
enum Timeline {
    ShrinkEvent {
        time: f64,
        location: Coordinate,
        left_vertex: IndexType,
        right_vertex: IndexType,
        left_real: usize,
        right_real: usize,
        tie_break: f64,
    },
    SplitEvent {
        time: f64,
        location: Coordinate,
        anchor_vertex: IndexType,
        anchor_real: usize,
    },
}

impl fmt::Display for Timeline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::ShrinkEvent {
                left_real,
                right_real,
                ..
            } => write!(f, "Shrink {} and {}", *left_real, *right_real),
            Self::SplitEvent { anchor_real, .. } => write!(f, "Split {}", *anchor_real),
        }
    }
}

impl PartialOrd for Timeline {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let t1 = match self {
            Self::ShrinkEvent { time, .. } | Self::SplitEvent { time, .. } => *time,
        };
        let t2 = match other {
            Self::ShrinkEvent { time, .. } | Self::SplitEvent { time, .. } => *time,
        };
        if fneq(t1, t2) {
            return Some(t1.partial_cmp(&t2).unwrap());
        }
        let x1 = match self {
            Self::ShrinkEvent {
                location,
                left_real,
                right_real,
                tie_break,
                ..
            } => (1, tie_break, location, left_real, right_real),
            Self::SplitEvent {
                location,
                anchor_real,
                ..
            } => (0, &0., location, anchor_real, anchor_real),
        };
        let x2 = match other {
            Self::ShrinkEvent {
                location,
                left_real,
                right_real,
                tie_break,
                ..
            } => (1, tie_break, location, left_real, right_real),
            Self::SplitEvent {
                location,
                anchor_real,
                ..
            } => (0, &0., location, anchor_real, anchor_real),
        };
        Some(x1.partial_cmp(&x2).unwrap())
    }
}

/// This module implements a core logic of the polygon buffering algorithm. In the normal cases, you don't need to know how this
/// module works, nor need to use this module.
pub struct Skeleton {
    ray_vector: Vec<VertexType>,
    event_queue: Vec<Event>,
    initial_vertex_queue: VertexQueue,
}

impl Skeleton {
    pub fn apply_vertex_queue(
        &self,
        vertex_queue: &VertexQueue,
        offset_distance: f64,
    ) -> MultiPolygon {
        let mut res = Vec::new();
        let mut lsv = Vec::new();
        let mut crdv = Vec::new();
        let mut cur_vidx = usize::MAX;
        for (vidx, _, idx) in vertex_queue.iter() {
            if vidx != cur_vidx {
                if cur_vidx < usize::MAX {
                    let mut ls = LineString::from(crdv);
                    ls.close();
                    lsv.push(ls);
                }
                cur_vidx = vidx;
                crdv = Vec::new();
            }
            let crd = self.ray_vector[idx]
                .unwrap_ray()
                .point_by_ratio(offset_distance - self.ray_vector[idx].unwrap_time());
            crdv.push(crd);
        }
        if cur_vidx < usize::MAX {
            let mut ls = LineString::from(crdv);
            ls.close();
            lsv.push(ls);
        }
        for ls in &lsv {
            if ls.winding_order() == Some(WindingOrder::CounterClockwise) {
                let p1: Polygon = Polygon::new(ls.clone(), vec![]);
                res.push(p1);
            }
        }
        for ls in &lsv {
            if ls.winding_order() == Some(WindingOrder::Clockwise) {
                for e in &mut res {
                    if e.contains(ls) {
                        e.interiors_push(ls.clone());
                        break;
                    }
                }
            }
        }
        MultiPolygon::new(res)
    }

    pub fn get_vertex_queue(&self, time_elapsed: f64) -> VertexQueue {
        let mut ret = self.initial_vertex_queue.clone();
        for e in &self.event_queue {
            if e.unwrap_time() <= time_elapsed {
                Self::apply_event(&mut ret, e);
                ret.cleanup();
            } else {
                break;
            }
        }
        ret
    }

    fn find_split_vertex(
        cv: IndexType,
        vertex_queue: &VertexQueue,
        vertex_vector: &[VertexType],
        is_init: bool,
        orient: bool,
    ) -> Vec<(f64, Coordinate, IndexType, usize)> {
        let mut ret = Vec::new();
        let cv_real = vertex_queue.get_real_index(cv);
        let left_ray = vertex_vector[cv_real].unwrap_base_ray().0;
        let right_ray = vertex_vector[cv_real].unwrap_base_ray().1;
        if orient && fleq(left_ray.angle.outer_product(&right_ray.angle), 0.) {
            return ret;
        } // check if ver_vec[i] is a reflex vertex
        if !orient && fgeq(left_ray.angle.outer_product(&right_ray.angle), 0.) {
            return ret;
        }

        for (_, sv, sv_real) in vertex_queue.iter() {
            let srv = vertex_queue.rv(sv);
            let srv_real = vertex_queue.get_real_index(srv);
            if sv == cv || sv == vertex_queue.rv(cv) || srv == cv || srv == vertex_queue.lv(cv) {
                continue;
            }
            let base_ray = vertex_vector[sv_real].unwrap_base_ray().1;
            let left_intersection = if left_ray.is_parallel(&base_ray) {
                Coordinate::default()
            } else {
                left_ray.intersect(&base_ray)
            };
            let right_intersection = if right_ray.is_parallel(&base_ray) {
                Coordinate::default()
            } else {
                right_ray.intersect(&base_ray)
            };
            let real_intersection = if left_ray.is_parallel(&base_ray) {
                let ri_ray = right_ray.bisector(&base_ray.reverse(), right_intersection, !orient);
                if !ri_ray.is_intersect(&vertex_vector[cv_real].unwrap_ray()) {
                    continue;
                }
                ri_ray.intersect(&vertex_vector[cv_real].unwrap_ray())
            } else {
                let li_ray = left_ray.bisector(&base_ray, left_intersection, orient);
                if !li_ray.is_intersect(&vertex_vector[cv_real].unwrap_ray()) {
                    continue;
                }
                li_ray.intersect(&vertex_vector[cv_real].unwrap_ray())
            };
            if is_init {
                if orient && base_ray.orientation(&real_intersection) < 0 {
                    continue;
                }
                if !orient && base_ray.orientation(&real_intersection) > 0 {
                    continue;
                }
            } else if orient {
                if vertex_vector[sv_real]
                    .unwrap_ray()
                    .orientation(&real_intersection)
                    >= 0
                {
                    continue;
                }
                if base_ray.orientation(&real_intersection) < 0 {
                    continue;
                }
                if vertex_vector[srv_real]
                    .unwrap_ray()
                    .orientation(&real_intersection)
                    < 0
                {
                    continue;
                }
            } else {
                if vertex_vector[sv_real]
                    .unwrap_ray()
                    .orientation(&real_intersection)
                    <= 0
                {
                    continue;
                }
                if base_ray.orientation(&real_intersection) > 0 {
                    continue;
                }
                if vertex_vector[srv_real]
                    .unwrap_ray()
                    .orientation(&real_intersection)
                    > 0
                {
                    continue;
                }
            }
            let dist = real_intersection.dist_ray(&right_ray);
            ret.push((dist, real_intersection, sv, sv_real));
        }
        ret.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if !is_init && !ret.is_empty() {
            ret = vec![ret[0]];
        }
        ret
    }

    fn make_split_event(
        cv: IndexType,
        vertex_queue: &VertexQueue,
        event_pq: &mut PriorityQueue<Timeline>,
        vertex_vector: &[VertexType],
        orient: bool,
    ) {
        let resv = Self::find_split_vertex(cv, vertex_queue, vertex_vector, true, orient);
        let cv_real = vertex_queue.get_real_index(cv);
        for (time, location, _, _) in resv {
            event_pq.insert(Timeline::SplitEvent {
                time,
                location,
                anchor_vertex: cv,
                anchor_real: cv_real,
            });
        }
    }

    fn make_shrink_event(
        cv: IndexType,
        vertex_queue: &VertexQueue,
        event_pq: &mut PriorityQueue<Timeline>,
        vertex_vector: &[VertexType],
        is_init: bool,
    ) {
        let mut lv = cv;
        if vertex_queue.rv(cv) == vertex_queue.lv(cv) {
            return;
        }
        for _ in 0..2 {
            let rv = vertex_queue.rv(lv);
            let lv_real = vertex_queue.get_real_index(lv);
            let rv_real = vertex_queue.get_real_index(rv);
            let lv_ray = vertex_vector[lv_real].unwrap_ray();
            let rv_ray = vertex_vector[rv_real].unwrap_ray();
            if lv_ray.is_intersect(&rv_ray) {
                let cp = lv_ray.intersect(&rv_ray);
                let dist = cp.dist_ray(&vertex_vector[lv_real].unwrap_base_ray().0);
                let tie_break = lv_ray.origin.dist_coord(&rv_ray.origin);
                event_pq.insert(Timeline::ShrinkEvent {
                    time: dist,
                    location: cp,
                    left_vertex: lv,
                    right_vertex: rv,
                    left_real: lv_real,
                    right_real: rv_real,
                    tie_break,
                });
            }
            if is_init {
                break;
            }
            lv = vertex_queue.lv(cv);
        }
    }

    fn apply_event(
        vertex_queue: &mut VertexQueue,
        event: &Event,
    ) -> (Option<IndexType>, Option<IndexType>) {
        if let Event::VertexEvent {
            merge_from,
            merge_to,
            ..
        } = event
        {
            let merge_from = IndexType::PointerIndex(*merge_from);
            let merge_to = IndexType::RealIndex(*merge_to);
            let cv = vertex_queue.remove_and_set(merge_from, merge_to);
            if vertex_queue.lv(cv) == vertex_queue.rv(cv) {
                let lv = vertex_queue.lv(cv);
                vertex_queue.content[lv.get_index()].done = true;
                vertex_queue.content[cv.get_index()].done = true;
                return (
                    Some(vertex_queue.content[vertex_queue.lv(cv).get_index()].index),
                    None,
                );
            }
            return (Some(cv), None);
        }
        if let Event::EdgeEvent {
            split_from,
            split_into,
            split_to_left,
            split_to_right,
            ..
        } = event
        {
            let split_from = IndexType::PointerIndex(*split_from);
            let split_into = IndexType::PointerIndex(*split_into);
            let split_to_left = IndexType::RealIndex(*split_to_left);
            let split_to_right = IndexType::RealIndex(*split_to_right);
            let ret =
                vertex_queue.split_and_set(split_from, split_into, split_to_left, split_to_right);
            vertex_queue.cleanup();
            return (Some(ret.0), Some(ret.1));
        }

        (None, None)
    }

    pub fn skeleton_of_polygon_vector(input_polygon_vector: &Vec<Polygon>, orient: bool) -> Self {
        let mut vertex_vector =
            VertexType::initialize_from_polygon_vector(input_polygon_vector, orient);
        let mut event_pq = PriorityQueue::new();
        let mut event_queue = Vec::new();
        let mut vertex_queue = VertexQueue::new();
        vertex_queue.initialize_from_polygon_vector(input_polygon_vector);
        let initial_vertex_queue = vertex_queue.clone();
        // make initial PQ
        for (_, cv, _) in vertex_queue.iter() {
            Self::make_shrink_event(cv, &vertex_queue, &mut event_pq, &vertex_vector, true);
            Self::make_split_event(cv, &vertex_queue, &mut event_pq, &vertex_vector, orient);
        }

        while !event_pq.is_empty() {
            let x = event_pq.pop().unwrap();
            if let Timeline::ShrinkEvent {
                time,
                location,
                left_vertex,
                right_vertex,
                left_real,
                right_real,
                ..
            } = x
            {
                if vertex_queue.content[left_vertex.get_index()].done
                    || vertex_queue.content[right_vertex.get_index()].done
                    || vertex_queue.get_real_index(left_vertex) != left_real
                    || vertex_queue.get_real_index(right_vertex) != right_real
                {
                    continue;
                }
                let new_index = vertex_vector.len();
                let left_ray = vertex_vector[left_real].unwrap_base_ray().0;
                let right_ray = vertex_vector[right_real].unwrap_base_ray().1;
                vertex_vector[left_real].set_parent(new_index);
                vertex_vector[right_real].set_parent(new_index);
                let new_event = Event::VertexEvent {
                    time,
                    merge_from: left_vertex.get_index(),
                    merge_to: new_index,
                };
                let new_vertex = VertexType::new_tree_vertex(location, left_ray, right_ray, orient);
                vertex_vector.push(new_vertex);
                match Self::apply_event(&mut vertex_queue, &new_event) {
                    (Some(IndexType::RealIndex(rv)), None) => {
                        vertex_vector[rv].set_parent(new_index);
                        vertex_vector[new_index] = VertexType::new_root_vertex(
                            vertex_vector[new_index].unwrap_location(),
                            vertex_vector[new_index].unwrap_time(),
                        );
                    }
                    (Some(cv), None) => {
                        Self::make_shrink_event(
                            cv,
                            &vertex_queue,
                            &mut event_pq,
                            &vertex_vector,
                            false,
                        );
                    }
                    _ => panic!("Expected Vertex Event"),
                }
                event_queue.push(new_event);
            } else if let Timeline::SplitEvent {
                time,
                location,
                anchor_vertex,
                anchor_real,
            } = x
            {
                if vertex_queue.content[anchor_vertex.get_index()].done
                    || vertex_queue.get_real_index(anchor_vertex) != anchor_real
                {
                    continue;
                }
                vertex_queue.cleanup();
                let rv = Self::find_split_vertex(
                    anchor_vertex,
                    &vertex_queue,
                    &vertex_vector,
                    false,
                    orient,
                );
                if rv.len() == 1 && feq(rv[0].0, time) && rv[0].1.eq(&location) {
                    let new_index1 = vertex_vector.len();
                    let new_index2 = new_index1 + 1;
                    let new_split_vertex = VertexType::new_split_vertex(
                        location,
                        vertex_vector[anchor_real].unwrap_time(),
                    );
                    let new_tree_vertex1 = VertexType::new_tree_vertex(
                        location,
                        vertex_vector[anchor_real].unwrap_base_ray().0,
                        vertex_vector[rv[0].3].unwrap_base_ray().1,
                        orient,
                    );
                    let new_tree_vertex2 = VertexType::new_tree_vertex(
                        location,
                        vertex_vector[rv[0].3].unwrap_base_ray().1.reverse(),
                        vertex_vector[anchor_real].unwrap_base_ray().1,
                        orient,
                    );
                    vertex_vector.push(new_tree_vertex1);
                    vertex_vector.push(new_tree_vertex2);
                    vertex_vector.push(new_split_vertex);
                    let new_event = Event::EdgeEvent {
                        time,
                        split_from: anchor_vertex.get_index(),
                        split_into: rv[0].2.get_index(),
                        split_to_left: new_index1,
                        split_to_right: new_index2,
                    };
                    match Self::apply_event(&mut vertex_queue, &new_event) {
                        (Some(cv1), Some(cv2)) => {
                            vertex_vector[anchor_real].set_parent(new_index2 + 1);
                            Self::make_shrink_event(
                                cv1,
                                &vertex_queue,
                                &mut event_pq,
                                &vertex_vector,
                                false,
                            );
                            Self::make_shrink_event(
                                cv2,
                                &vertex_queue,
                                &mut event_pq,
                                &vertex_vector,
                                false,
                            );
                        }
                        _ => panic!("Expected Edge Event"),
                    }
                    event_queue.push(new_event);
                }
            }
            vertex_queue.cleanup();
        }
        Self {
            ray_vector: vertex_vector,
            event_queue,
            initial_vertex_queue,
        }
    }
}
