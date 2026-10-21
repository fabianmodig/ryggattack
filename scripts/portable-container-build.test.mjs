// CI runs node:22-bookworm through the runner's Podman-compatible Docker API.
import fs from 'node:fs';
import assert from 'node:assert/strict';
const workflow=fs.readFileSync('.forgejo/workflows/release.yml','utf8');
assert.match(workflow,/DOCKER_BUILDKIT: "0"/,'use the portable legacy Docker API builder explicitly');
assert(!workflow.includes('docker-buildx'),'Bookworm release must not require unavailable buildx package');
for(const component of ['web','server']) {
 const dockerfile=fs.readFileSync(`${component}/Dockerfile`,'utf8');
 assert(!/COPY\s+--chmod=/.test(dockerfile),'set final permissions with portable RUN chmod');
 if(component==='web')assert.match(dockerfile,/chmod 0644 \/etc\/nginx\/nginx\.conf \/etc\/nginx\/conf\.d\/default\.conf/);
 else assert.match(dockerfile,/chmod 0555 \/usr\/local\/bin\/ryggattack-server/);
}
console.log('Portable Bookworm Docker build contract passed; image runtime checks remain required');
