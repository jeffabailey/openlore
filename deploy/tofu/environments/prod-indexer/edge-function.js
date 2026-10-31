// The only requests the index answers (D-STZ-21). Every behavior allows every method and this
// decides, so anything else is told 404 and never the 403 a narrower method set would give
// (U-STZ-4). The key is exact: a trailing slash or a prefix is not a match.
var ALLOWED = {
  'POST /xrpc/org.openlore.appview.searchClaims': true,
  'GET /healthz': true,
  'HEAD /healthz': true,
};

var NOT_FOUND = {
  statusCode: 404,
  statusDescription: 'Not Found',
  headers: { 'content-type': { value: 'text/plain' } },
  body: { encoding: 'text', data: 'not found\n' },
};

function handler(event) {
  var request = event.request;
  return ALLOWED[request.method + ' ' + request.uri] === true ? request : NOT_FOUND;
}
