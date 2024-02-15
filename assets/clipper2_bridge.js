import Clipper2ZFactory from "./clipper2z.js";

Clipper2ZFactory().then((Clipper2Z) => {
    window.Clipper2Z = Clipper2Z; // Expose Clipper2Z to the global scope
});

function offset_polygon(polygon, offsetSize, roundJoins) {
    const { Paths64, MakePath64, InflatePaths64, JoinType, EndType } = window.Clipper2Z;

    const subject = new Paths64();
    subject.push_back(MakePath64(polygon));

    // Paths64 InflatePaths(const Paths64& paths, double delta, JoinType join_type, EndType end_type, double miter_limit);
    const joinType = roundJoins ? JoinType.Round : JoinType.Miter;
    const inflated = InflatePaths64(subject, offsetSize, joinType, EndType.Polygon, 2, 0);

    const polygons = [];

    const size = inflated.size();
    for (let i = 0; i < size; i++) {
        const path = inflated.get(i);

        const size2 = path.size();
        const polygon = [];
        for (let j = 0; j < size2; j++) {
            const point = path.get(j);
            polygon.push(Number(point.x));
            polygon.push(Number(point.y));
        }
        polygons.push(polygon);
    }

    return polygons;
}

window.offset_polygon = offset_polygon;
