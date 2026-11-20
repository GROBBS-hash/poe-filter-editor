/** A page's content while it loads: two shimmering cards instead of a bare "Loading…". */
export function PageLoading() {
  return (
    <>
      <span className="skel" style={{ height: 120, borderRadius: 14, marginBottom: 14 }} />
      <span className="skel" style={{ height: 200, borderRadius: 14 }} />
    </>
  );
}
