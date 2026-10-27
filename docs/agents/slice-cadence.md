# Slice cadence: try → test → advance

A standing instruction for every slice that ships through the nWave pipeline. Advancing
is already approved: run the three steps straight through into the next slice's
pipeline. Pause only at a genuine scope fork (a real scope decision the user must
make), and put that one decision to the user.

1. **Try it.** Run a live demo of the shipped behavior against the real binary (e.g.
   spawn the real `ViewerServer` over a production-seeded store and exercise the new
   surface end-to-end).
   Done when every new surface of the slice has been exercised end-to-end and its
   output observed.
2. **Back it with tests.** Apply the Tests section of `CODING_STANDARDS.md` to every
   behavior the demo showed.
   Done when each demoed behavior meets that section.
3. **Advance.** Pick the strongest grounded next slice from the open jobs in
   `docs/product/jobs.yaml` and start its full nWave pipeline.
   Done when the next slice is named with a one-line reason and its pipeline has
   started.
