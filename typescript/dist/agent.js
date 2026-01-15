(function (exports) {
  'use strict';

  class FetchResponse {
    status;
    statusText;
    ok;
    headers;
    url;
    redirected;
    type;
    _body;
    _bodyUsed = false;
    constructor(status, body, url = "") {
      this.status = status;
      this.statusText = status >= 200 && status < 300 ? "OK" : "Error";
      this.ok = status >= 200 && status < 300;
      this.headers = new Headers();
      this.headers.set("content-type", "application/json");
      this.url = url;
      this.redirected = false;
      this.type = "basic";
      this._body = body;
    }
    get body() {
      return null;
    }
    get bodyUsed() {
      return this._bodyUsed;
    }
    async arrayBuffer() {
      this._bodyUsed = true;
      const encoder = new TextEncoder();
      return encoder.encode(this._body).buffer;
    }
    async blob() {
      this._bodyUsed = true;
      return new Blob([this._body], { type: "application/json" });
    }
    async formData() {
      throw new Error("formData() not implemented");
    }
    async json() {
      this._bodyUsed = true;
      return JSON.parse(this._body);
    }
    async text() {
      this._bodyUsed = true;
      return this._body;
    }
    clone() {
      return new FetchResponse(this.status, this._body, this.url);
    }
    async bytes() {
      this._bodyUsed = true;
      const encoder = new TextEncoder();
      return encoder.encode(this._body);
    }
  }
  if (typeof globalThis.Headers === "undefined") {
    globalThis.Headers = class HeadersPolyfill {
      _headers = /* @__PURE__ */ new Map();
      constructor(init) {
        if (init) {
          if (init instanceof HeadersPolyfill) {
            init._headers.forEach((value, key) => {
              this._headers.set(key.toLowerCase(), value);
            });
          } else if (Array.isArray(init)) {
            for (const [key, value] of init) {
              this._headers.set(key.toLowerCase(), value);
            }
          } else if (typeof init === "object") {
            for (const [key, value] of Object.entries(init)) {
              this._headers.set(key.toLowerCase(), value);
            }
          }
        }
      }
      append(name, value) {
        const existing = this._headers.get(name.toLowerCase());
        this._headers.set(name.toLowerCase(), existing ? `${existing}, ${value}` : value);
      }
      delete(name) {
        this._headers.delete(name.toLowerCase());
      }
      get(name) {
        return this._headers.get(name.toLowerCase()) ?? null;
      }
      has(name) {
        return this._headers.has(name.toLowerCase());
      }
      set(name, value) {
        this._headers.set(name.toLowerCase(), value);
      }
      forEach(callback) {
        this._headers.forEach((value, key) => {
          callback(value, key, this);
        });
      }
      entries() {
        return this._headers.entries();
      }
      keys() {
        return this._headers.keys();
      }
      values() {
        return this._headers.values();
      }
      [Symbol.iterator]() {
        return this._headers.entries();
      }
      getSetCookie() {
        const cookie = this._headers.get("set-cookie");
        return cookie ? [cookie] : [];
      }
    };
  }
  async function fetchViaOps(input, init) {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    const method = init?.method ?? "GET";
    const headers = {};
    if (init?.headers) {
      if (init.headers instanceof Headers) {
        init.headers.forEach((value, key) => {
          headers[key] = value;
        });
      } else if (Array.isArray(init.headers)) {
        for (const [key, value] of init.headers) {
          headers[key] = value;
        }
      } else {
        Object.assign(headers, init.headers);
      }
    }
    const body = init?.body ? String(init.body) : void 0;
    try {
      const response = await globalThis.Deno.core.ops.op_fetch(url, {
        method,
        headers,
        body
      });
      return new FetchResponse(response.status, response.body, url);
    } catch (error) {
      throw new TypeError(`Network request failed: ${error.message}`);
    }
  }
  if (typeof globalThis.fetch === "undefined") {
    globalThis.fetch = fetchViaOps;
  }
  if (typeof globalThis.Response === "undefined") {
    globalThis.Response = FetchResponse;
  }
  if (typeof globalThis.Request === "undefined") {
    globalThis.Request = class RequestPolyfill {
      url;
      method;
      headers;
      body = null;
      bodyUsed = false;
      cache = "default";
      credentials = "same-origin";
      destination = "";
      integrity = "";
      keepalive = false;
      mode = "cors";
      redirect = "follow";
      referrer = "";
      referrerPolicy = "";
      signal = new AbortController().signal;
      attribute = void 0;
      targetAddressSpace = void 0;
      constructor(input, init) {
        this.url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
        this.method = init?.method ?? "GET";
        this.headers = new Headers(init?.headers);
      }
      clone() {
        return new RequestPolyfill(this.url, {
          method: this.method,
          headers: this.headers
        });
      }
      async arrayBuffer() {
        return new ArrayBuffer(0);
      }
      async blob() {
        return new Blob();
      }
      async formData() {
        throw new Error("formData() not implemented");
      }
      async json() {
        return {};
      }
      async text() {
        return "";
      }
      async bytes() {
        return new Uint8Array();
      }
    };
  }

  let auto = false;
  let kind = undefined;
  let fetch$1 = undefined;
  let File$1 = undefined;
  let ReadableStream$1 = undefined;
  let getDefaultAgent = undefined;
  let fileFromPath = undefined;
  function setShims(shims, options = { auto: false }) {
      if (auto) {
          throw new Error(`you must \`import '@anthropic-ai/sdk/shims/${shims.kind}'\` before importing anything else from @anthropic-ai/sdk`);
      }
      if (kind) {
          throw new Error(`can't \`import '@anthropic-ai/sdk/shims/${shims.kind}'\` after \`import '@anthropic-ai/sdk/shims/${kind}'\``);
      }
      auto = options.auto;
      kind = shims.kind;
      fetch$1 = shims.fetch;
      shims.Request;
      shims.Response;
      shims.Headers;
      shims.FormData;
      shims.Blob;
      File$1 = shims.File;
      ReadableStream$1 = shims.ReadableStream;
      shims.getMultipartRequestOptions;
      getDefaultAgent = shims.getDefaultAgent;
      fileFromPath = shims.fileFromPath;
      shims.isFsReadStream;
  }

  /**
   * Disclaimer: modules in _shims aren't intended to be imported by SDK users.
   */
  class MultipartBody {
      constructor(body) {
          this.body = body;
      }
      get [Symbol.toStringTag]() {
          return 'MultipartBody';
      }
  }

  function getRuntime({ manuallyImported } = {}) {
      const recommendation = manuallyImported ?
          `You may need to use polyfills`
          : `Add one of these imports before your first \`import … from '@anthropic-ai/sdk'\`:
- \`import '@anthropic-ai/sdk/shims/node'\` (if you're running on Node)
- \`import '@anthropic-ai/sdk/shims/web'\` (otherwise)
`;
      let _fetch, _Request, _Response, _Headers;
      try {
          // @ts-ignore
          _fetch = fetch;
          // @ts-ignore
          _Request = Request;
          // @ts-ignore
          _Response = Response;
          // @ts-ignore
          _Headers = Headers;
      }
      catch (error) {
          throw new Error(`this environment is missing the following Web Fetch API type: ${error.message}. ${recommendation}`);
      }
      return {
          kind: 'web',
          fetch: _fetch,
          Request: _Request,
          Response: _Response,
          Headers: _Headers,
          FormData: 
          // @ts-ignore
          typeof FormData !== 'undefined' ? FormData : (class FormData {
              // @ts-ignore
              constructor() {
                  throw new Error(`file uploads aren't supported in this environment yet as 'FormData' is undefined. ${recommendation}`);
              }
          }),
          Blob: typeof Blob !== 'undefined' ? Blob : (class Blob {
              constructor() {
                  throw new Error(`file uploads aren't supported in this environment yet as 'Blob' is undefined. ${recommendation}`);
              }
          }),
          File: 
          // @ts-ignore
          typeof File !== 'undefined' ? File : (class File {
              // @ts-ignore
              constructor() {
                  throw new Error(`file uploads aren't supported in this environment yet as 'File' is undefined. ${recommendation}`);
              }
          }),
          ReadableStream: 
          // @ts-ignore
          typeof ReadableStream !== 'undefined' ? ReadableStream : (class ReadableStream {
              // @ts-ignore
              constructor() {
                  throw new Error(`streaming isn't supported in this environment yet as 'ReadableStream' is undefined. ${recommendation}`);
              }
          }),
          getMultipartRequestOptions: async (
          // @ts-ignore
          form, opts) => ({
              ...opts,
              body: new MultipartBody(form),
          }),
          getDefaultAgent: (url) => undefined,
          fileFromPath: () => {
              throw new Error('The `fileFromPath` function is only supported in Node. See the README for more details: https://www.github.com/anthropics/anthropic-sdk-typescript#file-uploads');
          },
          isFsReadStream: (value) => false,
      };
  }

  setShims(getRuntime({ manuallyImported: true }));

  function getDefaultExportFromCjs (x) {
  	return x && x.__esModule && Object.prototype.hasOwnProperty.call(x, 'default') ? x['default'] : x;
  }

  var buffer = {};

  var base64Js = {};

  var hasRequiredBase64Js;

  function requireBase64Js () {
  	if (hasRequiredBase64Js) return base64Js;
  	hasRequiredBase64Js = 1;

  	base64Js.byteLength = byteLength;
  	base64Js.toByteArray = toByteArray;
  	base64Js.fromByteArray = fromByteArray;

  	var lookup = [];
  	var revLookup = [];
  	var Arr = typeof Uint8Array !== 'undefined' ? Uint8Array : Array;

  	var code = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  	for (var i = 0, len = code.length; i < len; ++i) {
  	  lookup[i] = code[i];
  	  revLookup[code.charCodeAt(i)] = i;
  	}

  	// Support decoding URL-safe base64 strings, as Node.js does.
  	// See: https://en.wikipedia.org/wiki/Base64#URL_applications
  	revLookup['-'.charCodeAt(0)] = 62;
  	revLookup['_'.charCodeAt(0)] = 63;

  	function getLens (b64) {
  	  var len = b64.length;

  	  if (len % 4 > 0) {
  	    throw new Error('Invalid string. Length must be a multiple of 4')
  	  }

  	  // Trim off extra bytes after placeholder bytes are found
  	  // See: https://github.com/beatgammit/base64-js/issues/42
  	  var validLen = b64.indexOf('=');
  	  if (validLen === -1) validLen = len;

  	  var placeHoldersLen = validLen === len
  	    ? 0
  	    : 4 - (validLen % 4);

  	  return [validLen, placeHoldersLen]
  	}

  	// base64 is 4/3 + up to two characters of the original data
  	function byteLength (b64) {
  	  var lens = getLens(b64);
  	  var validLen = lens[0];
  	  var placeHoldersLen = lens[1];
  	  return ((validLen + placeHoldersLen) * 3 / 4) - placeHoldersLen
  	}

  	function _byteLength (b64, validLen, placeHoldersLen) {
  	  return ((validLen + placeHoldersLen) * 3 / 4) - placeHoldersLen
  	}

  	function toByteArray (b64) {
  	  var tmp;
  	  var lens = getLens(b64);
  	  var validLen = lens[0];
  	  var placeHoldersLen = lens[1];

  	  var arr = new Arr(_byteLength(b64, validLen, placeHoldersLen));

  	  var curByte = 0;

  	  // if there are placeholders, only get up to the last complete 4 chars
  	  var len = placeHoldersLen > 0
  	    ? validLen - 4
  	    : validLen;

  	  var i;
  	  for (i = 0; i < len; i += 4) {
  	    tmp =
  	      (revLookup[b64.charCodeAt(i)] << 18) |
  	      (revLookup[b64.charCodeAt(i + 1)] << 12) |
  	      (revLookup[b64.charCodeAt(i + 2)] << 6) |
  	      revLookup[b64.charCodeAt(i + 3)];
  	    arr[curByte++] = (tmp >> 16) & 0xFF;
  	    arr[curByte++] = (tmp >> 8) & 0xFF;
  	    arr[curByte++] = tmp & 0xFF;
  	  }

  	  if (placeHoldersLen === 2) {
  	    tmp =
  	      (revLookup[b64.charCodeAt(i)] << 2) |
  	      (revLookup[b64.charCodeAt(i + 1)] >> 4);
  	    arr[curByte++] = tmp & 0xFF;
  	  }

  	  if (placeHoldersLen === 1) {
  	    tmp =
  	      (revLookup[b64.charCodeAt(i)] << 10) |
  	      (revLookup[b64.charCodeAt(i + 1)] << 4) |
  	      (revLookup[b64.charCodeAt(i + 2)] >> 2);
  	    arr[curByte++] = (tmp >> 8) & 0xFF;
  	    arr[curByte++] = tmp & 0xFF;
  	  }

  	  return arr
  	}

  	function tripletToBase64 (num) {
  	  return lookup[num >> 18 & 0x3F] +
  	    lookup[num >> 12 & 0x3F] +
  	    lookup[num >> 6 & 0x3F] +
  	    lookup[num & 0x3F]
  	}

  	function encodeChunk (uint8, start, end) {
  	  var tmp;
  	  var output = [];
  	  for (var i = start; i < end; i += 3) {
  	    tmp =
  	      ((uint8[i] << 16) & 0xFF0000) +
  	      ((uint8[i + 1] << 8) & 0xFF00) +
  	      (uint8[i + 2] & 0xFF);
  	    output.push(tripletToBase64(tmp));
  	  }
  	  return output.join('')
  	}

  	function fromByteArray (uint8) {
  	  var tmp;
  	  var len = uint8.length;
  	  var extraBytes = len % 3; // if we have 1 byte left, pad 2 bytes
  	  var parts = [];
  	  var maxChunkLength = 16383; // must be multiple of 3

  	  // go through the array every three bytes, we'll deal with trailing stuff later
  	  for (var i = 0, len2 = len - extraBytes; i < len2; i += maxChunkLength) {
  	    parts.push(encodeChunk(uint8, i, (i + maxChunkLength) > len2 ? len2 : (i + maxChunkLength)));
  	  }

  	  // pad the end with zeros, but make sure to not forget the extra bytes
  	  if (extraBytes === 1) {
  	    tmp = uint8[len - 1];
  	    parts.push(
  	      lookup[tmp >> 2] +
  	      lookup[(tmp << 4) & 0x3F] +
  	      '=='
  	    );
  	  } else if (extraBytes === 2) {
  	    tmp = (uint8[len - 2] << 8) + uint8[len - 1];
  	    parts.push(
  	      lookup[tmp >> 10] +
  	      lookup[(tmp >> 4) & 0x3F] +
  	      lookup[(tmp << 2) & 0x3F] +
  	      '='
  	    );
  	  }

  	  return parts.join('')
  	}
  	return base64Js;
  }

  var ieee754 = {};

  /*! ieee754. BSD-3-Clause License. Feross Aboukhadijeh <https://feross.org/opensource> */

  var hasRequiredIeee754;

  function requireIeee754 () {
  	if (hasRequiredIeee754) return ieee754;
  	hasRequiredIeee754 = 1;
  	ieee754.read = function (buffer, offset, isLE, mLen, nBytes) {
  	  var e, m;
  	  var eLen = (nBytes * 8) - mLen - 1;
  	  var eMax = (1 << eLen) - 1;
  	  var eBias = eMax >> 1;
  	  var nBits = -7;
  	  var i = isLE ? (nBytes - 1) : 0;
  	  var d = isLE ? -1 : 1;
  	  var s = buffer[offset + i];

  	  i += d;

  	  e = s & ((1 << (-nBits)) - 1);
  	  s >>= (-nBits);
  	  nBits += eLen;
  	  for (; nBits > 0; e = (e * 256) + buffer[offset + i], i += d, nBits -= 8) {}

  	  m = e & ((1 << (-nBits)) - 1);
  	  e >>= (-nBits);
  	  nBits += mLen;
  	  for (; nBits > 0; m = (m * 256) + buffer[offset + i], i += d, nBits -= 8) {}

  	  if (e === 0) {
  	    e = 1 - eBias;
  	  } else if (e === eMax) {
  	    return m ? NaN : ((s ? -1 : 1) * Infinity)
  	  } else {
  	    m = m + Math.pow(2, mLen);
  	    e = e - eBias;
  	  }
  	  return (s ? -1 : 1) * m * Math.pow(2, e - mLen)
  	};

  	ieee754.write = function (buffer, value, offset, isLE, mLen, nBytes) {
  	  var e, m, c;
  	  var eLen = (nBytes * 8) - mLen - 1;
  	  var eMax = (1 << eLen) - 1;
  	  var eBias = eMax >> 1;
  	  var rt = (mLen === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0);
  	  var i = isLE ? 0 : (nBytes - 1);
  	  var d = isLE ? 1 : -1;
  	  var s = value < 0 || (value === 0 && 1 / value < 0) ? 1 : 0;

  	  value = Math.abs(value);

  	  if (isNaN(value) || value === Infinity) {
  	    m = isNaN(value) ? 1 : 0;
  	    e = eMax;
  	  } else {
  	    e = Math.floor(Math.log(value) / Math.LN2);
  	    if (value * (c = Math.pow(2, -e)) < 1) {
  	      e--;
  	      c *= 2;
  	    }
  	    if (e + eBias >= 1) {
  	      value += rt / c;
  	    } else {
  	      value += rt * Math.pow(2, 1 - eBias);
  	    }
  	    if (value * c >= 2) {
  	      e++;
  	      c /= 2;
  	    }

  	    if (e + eBias >= eMax) {
  	      m = 0;
  	      e = eMax;
  	    } else if (e + eBias >= 1) {
  	      m = ((value * c) - 1) * Math.pow(2, mLen);
  	      e = e + eBias;
  	    } else {
  	      m = value * Math.pow(2, eBias - 1) * Math.pow(2, mLen);
  	      e = 0;
  	    }
  	  }

  	  for (; mLen >= 8; buffer[offset + i] = m & 0xff, i += d, m /= 256, mLen -= 8) {}

  	  e = (e << mLen) | m;
  	  eLen += mLen;
  	  for (; eLen > 0; buffer[offset + i] = e & 0xff, i += d, e /= 256, eLen -= 8) {}

  	  buffer[offset + i - d] |= s * 128;
  	};
  	return ieee754;
  }

  /*!
   * The buffer module from node.js, for the browser.
   *
   * @author   Feross Aboukhadijeh <https://feross.org>
   * @license  MIT
   */

  var hasRequiredBuffer;

  function requireBuffer () {
  	if (hasRequiredBuffer) return buffer;
  	hasRequiredBuffer = 1;
  	(function (exports$1) {

  		var base64 = requireBase64Js();
  		var ieee754 = requireIeee754();
  		var customInspectSymbol =
  		  (typeof Symbol === 'function' && typeof Symbol['for'] === 'function') // eslint-disable-line dot-notation
  		    ? Symbol['for']('nodejs.util.inspect.custom') // eslint-disable-line dot-notation
  		    : null;

  		exports$1.Buffer = Buffer;
  		exports$1.SlowBuffer = SlowBuffer;
  		exports$1.INSPECT_MAX_BYTES = 50;

  		var K_MAX_LENGTH = 0x7fffffff;
  		exports$1.kMaxLength = K_MAX_LENGTH;

  		/**
  		 * If `Buffer.TYPED_ARRAY_SUPPORT`:
  		 *   === true    Use Uint8Array implementation (fastest)
  		 *   === false   Print warning and recommend using `buffer` v4.x which has an Object
  		 *               implementation (most compatible, even IE6)
  		 *
  		 * Browsers that support typed arrays are IE 10+, Firefox 4+, Chrome 7+, Safari 5.1+,
  		 * Opera 11.6+, iOS 4.2+.
  		 *
  		 * We report that the browser does not support typed arrays if the are not subclassable
  		 * using __proto__. Firefox 4-29 lacks support for adding new properties to `Uint8Array`
  		 * (See: https://bugzilla.mozilla.org/show_bug.cgi?id=695438). IE 10 lacks support
  		 * for __proto__ and has a buggy typed array implementation.
  		 */
  		Buffer.TYPED_ARRAY_SUPPORT = typedArraySupport();

  		if (!Buffer.TYPED_ARRAY_SUPPORT && typeof console !== 'undefined' &&
  		    typeof console.error === 'function') {
  		  console.error(
  		    'This browser lacks typed array (Uint8Array) support which is required by ' +
  		    '`buffer` v5.x. Use `buffer` v4.x if you require old browser support.'
  		  );
  		}

  		function typedArraySupport () {
  		  // Can typed array instances can be augmented?
  		  try {
  		    var arr = new Uint8Array(1);
  		    var proto = { foo: function () { return 42 } };
  		    Object.setPrototypeOf(proto, Uint8Array.prototype);
  		    Object.setPrototypeOf(arr, proto);
  		    return arr.foo() === 42
  		  } catch (e) {
  		    return false
  		  }
  		}

  		Object.defineProperty(Buffer.prototype, 'parent', {
  		  enumerable: true,
  		  get: function () {
  		    if (!Buffer.isBuffer(this)) return undefined
  		    return this.buffer
  		  }
  		});

  		Object.defineProperty(Buffer.prototype, 'offset', {
  		  enumerable: true,
  		  get: function () {
  		    if (!Buffer.isBuffer(this)) return undefined
  		    return this.byteOffset
  		  }
  		});

  		function createBuffer (length) {
  		  if (length > K_MAX_LENGTH) {
  		    throw new RangeError('The value "' + length + '" is invalid for option "size"')
  		  }
  		  // Return an augmented `Uint8Array` instance
  		  var buf = new Uint8Array(length);
  		  Object.setPrototypeOf(buf, Buffer.prototype);
  		  return buf
  		}

  		/**
  		 * The Buffer constructor returns instances of `Uint8Array` that have their
  		 * prototype changed to `Buffer.prototype`. Furthermore, `Buffer` is a subclass of
  		 * `Uint8Array`, so the returned instances will have all the node `Buffer` methods
  		 * and the `Uint8Array` methods. Square bracket notation works as expected -- it
  		 * returns a single octet.
  		 *
  		 * The `Uint8Array` prototype remains unmodified.
  		 */

  		function Buffer (arg, encodingOrOffset, length) {
  		  // Common case.
  		  if (typeof arg === 'number') {
  		    if (typeof encodingOrOffset === 'string') {
  		      throw new TypeError(
  		        'The "string" argument must be of type string. Received type number'
  		      )
  		    }
  		    return allocUnsafe(arg)
  		  }
  		  return from(arg, encodingOrOffset, length)
  		}

  		Buffer.poolSize = 8192; // not used by this implementation

  		function from (value, encodingOrOffset, length) {
  		  if (typeof value === 'string') {
  		    return fromString(value, encodingOrOffset)
  		  }

  		  if (ArrayBuffer.isView(value)) {
  		    return fromArrayView(value)
  		  }

  		  if (value == null) {
  		    throw new TypeError(
  		      'The first argument must be one of type string, Buffer, ArrayBuffer, Array, ' +
  		      'or Array-like Object. Received type ' + (typeof value)
  		    )
  		  }

  		  if (isInstance(value, ArrayBuffer) ||
  		      (value && isInstance(value.buffer, ArrayBuffer))) {
  		    return fromArrayBuffer(value, encodingOrOffset, length)
  		  }

  		  if (typeof SharedArrayBuffer !== 'undefined' &&
  		      (isInstance(value, SharedArrayBuffer) ||
  		      (value && isInstance(value.buffer, SharedArrayBuffer)))) {
  		    return fromArrayBuffer(value, encodingOrOffset, length)
  		  }

  		  if (typeof value === 'number') {
  		    throw new TypeError(
  		      'The "value" argument must not be of type number. Received type number'
  		    )
  		  }

  		  var valueOf = value.valueOf && value.valueOf();
  		  if (valueOf != null && valueOf !== value) {
  		    return Buffer.from(valueOf, encodingOrOffset, length)
  		  }

  		  var b = fromObject(value);
  		  if (b) return b

  		  if (typeof Symbol !== 'undefined' && Symbol.toPrimitive != null &&
  		      typeof value[Symbol.toPrimitive] === 'function') {
  		    return Buffer.from(
  		      value[Symbol.toPrimitive]('string'), encodingOrOffset, length
  		    )
  		  }

  		  throw new TypeError(
  		    'The first argument must be one of type string, Buffer, ArrayBuffer, Array, ' +
  		    'or Array-like Object. Received type ' + (typeof value)
  		  )
  		}

  		/**
  		 * Functionally equivalent to Buffer(arg, encoding) but throws a TypeError
  		 * if value is a number.
  		 * Buffer.from(str[, encoding])
  		 * Buffer.from(array)
  		 * Buffer.from(buffer)
  		 * Buffer.from(arrayBuffer[, byteOffset[, length]])
  		 **/
  		Buffer.from = function (value, encodingOrOffset, length) {
  		  return from(value, encodingOrOffset, length)
  		};

  		// Note: Change prototype *after* Buffer.from is defined to workaround Chrome bug:
  		// https://github.com/feross/buffer/pull/148
  		Object.setPrototypeOf(Buffer.prototype, Uint8Array.prototype);
  		Object.setPrototypeOf(Buffer, Uint8Array);

  		function assertSize (size) {
  		  if (typeof size !== 'number') {
  		    throw new TypeError('"size" argument must be of type number')
  		  } else if (size < 0) {
  		    throw new RangeError('The value "' + size + '" is invalid for option "size"')
  		  }
  		}

  		function alloc (size, fill, encoding) {
  		  assertSize(size);
  		  if (size <= 0) {
  		    return createBuffer(size)
  		  }
  		  if (fill !== undefined) {
  		    // Only pay attention to encoding if it's a string. This
  		    // prevents accidentally sending in a number that would
  		    // be interpreted as a start offset.
  		    return typeof encoding === 'string'
  		      ? createBuffer(size).fill(fill, encoding)
  		      : createBuffer(size).fill(fill)
  		  }
  		  return createBuffer(size)
  		}

  		/**
  		 * Creates a new filled Buffer instance.
  		 * alloc(size[, fill[, encoding]])
  		 **/
  		Buffer.alloc = function (size, fill, encoding) {
  		  return alloc(size, fill, encoding)
  		};

  		function allocUnsafe (size) {
  		  assertSize(size);
  		  return createBuffer(size < 0 ? 0 : checked(size) | 0)
  		}

  		/**
  		 * Equivalent to Buffer(num), by default creates a non-zero-filled Buffer instance.
  		 * */
  		Buffer.allocUnsafe = function (size) {
  		  return allocUnsafe(size)
  		};
  		/**
  		 * Equivalent to SlowBuffer(num), by default creates a non-zero-filled Buffer instance.
  		 */
  		Buffer.allocUnsafeSlow = function (size) {
  		  return allocUnsafe(size)
  		};

  		function fromString (string, encoding) {
  		  if (typeof encoding !== 'string' || encoding === '') {
  		    encoding = 'utf8';
  		  }

  		  if (!Buffer.isEncoding(encoding)) {
  		    throw new TypeError('Unknown encoding: ' + encoding)
  		  }

  		  var length = byteLength(string, encoding) | 0;
  		  var buf = createBuffer(length);

  		  var actual = buf.write(string, encoding);

  		  if (actual !== length) {
  		    // Writing a hex string, for example, that contains invalid characters will
  		    // cause everything after the first invalid character to be ignored. (e.g.
  		    // 'abxxcd' will be treated as 'ab')
  		    buf = buf.slice(0, actual);
  		  }

  		  return buf
  		}

  		function fromArrayLike (array) {
  		  var length = array.length < 0 ? 0 : checked(array.length) | 0;
  		  var buf = createBuffer(length);
  		  for (var i = 0; i < length; i += 1) {
  		    buf[i] = array[i] & 255;
  		  }
  		  return buf
  		}

  		function fromArrayView (arrayView) {
  		  if (isInstance(arrayView, Uint8Array)) {
  		    var copy = new Uint8Array(arrayView);
  		    return fromArrayBuffer(copy.buffer, copy.byteOffset, copy.byteLength)
  		  }
  		  return fromArrayLike(arrayView)
  		}

  		function fromArrayBuffer (array, byteOffset, length) {
  		  if (byteOffset < 0 || array.byteLength < byteOffset) {
  		    throw new RangeError('"offset" is outside of buffer bounds')
  		  }

  		  if (array.byteLength < byteOffset + (length || 0)) {
  		    throw new RangeError('"length" is outside of buffer bounds')
  		  }

  		  var buf;
  		  if (byteOffset === undefined && length === undefined) {
  		    buf = new Uint8Array(array);
  		  } else if (length === undefined) {
  		    buf = new Uint8Array(array, byteOffset);
  		  } else {
  		    buf = new Uint8Array(array, byteOffset, length);
  		  }

  		  // Return an augmented `Uint8Array` instance
  		  Object.setPrototypeOf(buf, Buffer.prototype);

  		  return buf
  		}

  		function fromObject (obj) {
  		  if (Buffer.isBuffer(obj)) {
  		    var len = checked(obj.length) | 0;
  		    var buf = createBuffer(len);

  		    if (buf.length === 0) {
  		      return buf
  		    }

  		    obj.copy(buf, 0, 0, len);
  		    return buf
  		  }

  		  if (obj.length !== undefined) {
  		    if (typeof obj.length !== 'number' || numberIsNaN(obj.length)) {
  		      return createBuffer(0)
  		    }
  		    return fromArrayLike(obj)
  		  }

  		  if (obj.type === 'Buffer' && Array.isArray(obj.data)) {
  		    return fromArrayLike(obj.data)
  		  }
  		}

  		function checked (length) {
  		  // Note: cannot use `length < K_MAX_LENGTH` here because that fails when
  		  // length is NaN (which is otherwise coerced to zero.)
  		  if (length >= K_MAX_LENGTH) {
  		    throw new RangeError('Attempt to allocate Buffer larger than maximum ' +
  		                         'size: 0x' + K_MAX_LENGTH.toString(16) + ' bytes')
  		  }
  		  return length | 0
  		}

  		function SlowBuffer (length) {
  		  if (+length != length) { // eslint-disable-line eqeqeq
  		    length = 0;
  		  }
  		  return Buffer.alloc(+length)
  		}

  		Buffer.isBuffer = function isBuffer (b) {
  		  return b != null && b._isBuffer === true &&
  		    b !== Buffer.prototype // so Buffer.isBuffer(Buffer.prototype) will be false
  		};

  		Buffer.compare = function compare (a, b) {
  		  if (isInstance(a, Uint8Array)) a = Buffer.from(a, a.offset, a.byteLength);
  		  if (isInstance(b, Uint8Array)) b = Buffer.from(b, b.offset, b.byteLength);
  		  if (!Buffer.isBuffer(a) || !Buffer.isBuffer(b)) {
  		    throw new TypeError(
  		      'The "buf1", "buf2" arguments must be one of type Buffer or Uint8Array'
  		    )
  		  }

  		  if (a === b) return 0

  		  var x = a.length;
  		  var y = b.length;

  		  for (var i = 0, len = Math.min(x, y); i < len; ++i) {
  		    if (a[i] !== b[i]) {
  		      x = a[i];
  		      y = b[i];
  		      break
  		    }
  		  }

  		  if (x < y) return -1
  		  if (y < x) return 1
  		  return 0
  		};

  		Buffer.isEncoding = function isEncoding (encoding) {
  		  switch (String(encoding).toLowerCase()) {
  		    case 'hex':
  		    case 'utf8':
  		    case 'utf-8':
  		    case 'ascii':
  		    case 'latin1':
  		    case 'binary':
  		    case 'base64':
  		    case 'ucs2':
  		    case 'ucs-2':
  		    case 'utf16le':
  		    case 'utf-16le':
  		      return true
  		    default:
  		      return false
  		  }
  		};

  		Buffer.concat = function concat (list, length) {
  		  if (!Array.isArray(list)) {
  		    throw new TypeError('"list" argument must be an Array of Buffers')
  		  }

  		  if (list.length === 0) {
  		    return Buffer.alloc(0)
  		  }

  		  var i;
  		  if (length === undefined) {
  		    length = 0;
  		    for (i = 0; i < list.length; ++i) {
  		      length += list[i].length;
  		    }
  		  }

  		  var buffer = Buffer.allocUnsafe(length);
  		  var pos = 0;
  		  for (i = 0; i < list.length; ++i) {
  		    var buf = list[i];
  		    if (isInstance(buf, Uint8Array)) {
  		      if (pos + buf.length > buffer.length) {
  		        Buffer.from(buf).copy(buffer, pos);
  		      } else {
  		        Uint8Array.prototype.set.call(
  		          buffer,
  		          buf,
  		          pos
  		        );
  		      }
  		    } else if (!Buffer.isBuffer(buf)) {
  		      throw new TypeError('"list" argument must be an Array of Buffers')
  		    } else {
  		      buf.copy(buffer, pos);
  		    }
  		    pos += buf.length;
  		  }
  		  return buffer
  		};

  		function byteLength (string, encoding) {
  		  if (Buffer.isBuffer(string)) {
  		    return string.length
  		  }
  		  if (ArrayBuffer.isView(string) || isInstance(string, ArrayBuffer)) {
  		    return string.byteLength
  		  }
  		  if (typeof string !== 'string') {
  		    throw new TypeError(
  		      'The "string" argument must be one of type string, Buffer, or ArrayBuffer. ' +
  		      'Received type ' + typeof string
  		    )
  		  }

  		  var len = string.length;
  		  var mustMatch = (arguments.length > 2 && arguments[2] === true);
  		  if (!mustMatch && len === 0) return 0

  		  // Use a for loop to avoid recursion
  		  var loweredCase = false;
  		  for (;;) {
  		    switch (encoding) {
  		      case 'ascii':
  		      case 'latin1':
  		      case 'binary':
  		        return len
  		      case 'utf8':
  		      case 'utf-8':
  		        return utf8ToBytes(string).length
  		      case 'ucs2':
  		      case 'ucs-2':
  		      case 'utf16le':
  		      case 'utf-16le':
  		        return len * 2
  		      case 'hex':
  		        return len >>> 1
  		      case 'base64':
  		        return base64ToBytes(string).length
  		      default:
  		        if (loweredCase) {
  		          return mustMatch ? -1 : utf8ToBytes(string).length // assume utf8
  		        }
  		        encoding = ('' + encoding).toLowerCase();
  		        loweredCase = true;
  		    }
  		  }
  		}
  		Buffer.byteLength = byteLength;

  		function slowToString (encoding, start, end) {
  		  var loweredCase = false;

  		  // No need to verify that "this.length <= MAX_UINT32" since it's a read-only
  		  // property of a typed array.

  		  // This behaves neither like String nor Uint8Array in that we set start/end
  		  // to their upper/lower bounds if the value passed is out of range.
  		  // undefined is handled specially as per ECMA-262 6th Edition,
  		  // Section 13.3.3.7 Runtime Semantics: KeyedBindingInitialization.
  		  if (start === undefined || start < 0) {
  		    start = 0;
  		  }
  		  // Return early if start > this.length. Done here to prevent potential uint32
  		  // coercion fail below.
  		  if (start > this.length) {
  		    return ''
  		  }

  		  if (end === undefined || end > this.length) {
  		    end = this.length;
  		  }

  		  if (end <= 0) {
  		    return ''
  		  }

  		  // Force coercion to uint32. This will also coerce falsey/NaN values to 0.
  		  end >>>= 0;
  		  start >>>= 0;

  		  if (end <= start) {
  		    return ''
  		  }

  		  if (!encoding) encoding = 'utf8';

  		  while (true) {
  		    switch (encoding) {
  		      case 'hex':
  		        return hexSlice(this, start, end)

  		      case 'utf8':
  		      case 'utf-8':
  		        return utf8Slice(this, start, end)

  		      case 'ascii':
  		        return asciiSlice(this, start, end)

  		      case 'latin1':
  		      case 'binary':
  		        return latin1Slice(this, start, end)

  		      case 'base64':
  		        return base64Slice(this, start, end)

  		      case 'ucs2':
  		      case 'ucs-2':
  		      case 'utf16le':
  		      case 'utf-16le':
  		        return utf16leSlice(this, start, end)

  		      default:
  		        if (loweredCase) throw new TypeError('Unknown encoding: ' + encoding)
  		        encoding = (encoding + '').toLowerCase();
  		        loweredCase = true;
  		    }
  		  }
  		}

  		// This property is used by `Buffer.isBuffer` (and the `is-buffer` npm package)
  		// to detect a Buffer instance. It's not possible to use `instanceof Buffer`
  		// reliably in a browserify context because there could be multiple different
  		// copies of the 'buffer' package in use. This method works even for Buffer
  		// instances that were created from another copy of the `buffer` package.
  		// See: https://github.com/feross/buffer/issues/154
  		Buffer.prototype._isBuffer = true;

  		function swap (b, n, m) {
  		  var i = b[n];
  		  b[n] = b[m];
  		  b[m] = i;
  		}

  		Buffer.prototype.swap16 = function swap16 () {
  		  var len = this.length;
  		  if (len % 2 !== 0) {
  		    throw new RangeError('Buffer size must be a multiple of 16-bits')
  		  }
  		  for (var i = 0; i < len; i += 2) {
  		    swap(this, i, i + 1);
  		  }
  		  return this
  		};

  		Buffer.prototype.swap32 = function swap32 () {
  		  var len = this.length;
  		  if (len % 4 !== 0) {
  		    throw new RangeError('Buffer size must be a multiple of 32-bits')
  		  }
  		  for (var i = 0; i < len; i += 4) {
  		    swap(this, i, i + 3);
  		    swap(this, i + 1, i + 2);
  		  }
  		  return this
  		};

  		Buffer.prototype.swap64 = function swap64 () {
  		  var len = this.length;
  		  if (len % 8 !== 0) {
  		    throw new RangeError('Buffer size must be a multiple of 64-bits')
  		  }
  		  for (var i = 0; i < len; i += 8) {
  		    swap(this, i, i + 7);
  		    swap(this, i + 1, i + 6);
  		    swap(this, i + 2, i + 5);
  		    swap(this, i + 3, i + 4);
  		  }
  		  return this
  		};

  		Buffer.prototype.toString = function toString () {
  		  var length = this.length;
  		  if (length === 0) return ''
  		  if (arguments.length === 0) return utf8Slice(this, 0, length)
  		  return slowToString.apply(this, arguments)
  		};

  		Buffer.prototype.toLocaleString = Buffer.prototype.toString;

  		Buffer.prototype.equals = function equals (b) {
  		  if (!Buffer.isBuffer(b)) throw new TypeError('Argument must be a Buffer')
  		  if (this === b) return true
  		  return Buffer.compare(this, b) === 0
  		};

  		Buffer.prototype.inspect = function inspect () {
  		  var str = '';
  		  var max = exports$1.INSPECT_MAX_BYTES;
  		  str = this.toString('hex', 0, max).replace(/(.{2})/g, '$1 ').trim();
  		  if (this.length > max) str += ' ... ';
  		  return '<Buffer ' + str + '>'
  		};
  		if (customInspectSymbol) {
  		  Buffer.prototype[customInspectSymbol] = Buffer.prototype.inspect;
  		}

  		Buffer.prototype.compare = function compare (target, start, end, thisStart, thisEnd) {
  		  if (isInstance(target, Uint8Array)) {
  		    target = Buffer.from(target, target.offset, target.byteLength);
  		  }
  		  if (!Buffer.isBuffer(target)) {
  		    throw new TypeError(
  		      'The "target" argument must be one of type Buffer or Uint8Array. ' +
  		      'Received type ' + (typeof target)
  		    )
  		  }

  		  if (start === undefined) {
  		    start = 0;
  		  }
  		  if (end === undefined) {
  		    end = target ? target.length : 0;
  		  }
  		  if (thisStart === undefined) {
  		    thisStart = 0;
  		  }
  		  if (thisEnd === undefined) {
  		    thisEnd = this.length;
  		  }

  		  if (start < 0 || end > target.length || thisStart < 0 || thisEnd > this.length) {
  		    throw new RangeError('out of range index')
  		  }

  		  if (thisStart >= thisEnd && start >= end) {
  		    return 0
  		  }
  		  if (thisStart >= thisEnd) {
  		    return -1
  		  }
  		  if (start >= end) {
  		    return 1
  		  }

  		  start >>>= 0;
  		  end >>>= 0;
  		  thisStart >>>= 0;
  		  thisEnd >>>= 0;

  		  if (this === target) return 0

  		  var x = thisEnd - thisStart;
  		  var y = end - start;
  		  var len = Math.min(x, y);

  		  var thisCopy = this.slice(thisStart, thisEnd);
  		  var targetCopy = target.slice(start, end);

  		  for (var i = 0; i < len; ++i) {
  		    if (thisCopy[i] !== targetCopy[i]) {
  		      x = thisCopy[i];
  		      y = targetCopy[i];
  		      break
  		    }
  		  }

  		  if (x < y) return -1
  		  if (y < x) return 1
  		  return 0
  		};

  		// Finds either the first index of `val` in `buffer` at offset >= `byteOffset`,
  		// OR the last index of `val` in `buffer` at offset <= `byteOffset`.
  		//
  		// Arguments:
  		// - buffer - a Buffer to search
  		// - val - a string, Buffer, or number
  		// - byteOffset - an index into `buffer`; will be clamped to an int32
  		// - encoding - an optional encoding, relevant is val is a string
  		// - dir - true for indexOf, false for lastIndexOf
  		function bidirectionalIndexOf (buffer, val, byteOffset, encoding, dir) {
  		  // Empty buffer means no match
  		  if (buffer.length === 0) return -1

  		  // Normalize byteOffset
  		  if (typeof byteOffset === 'string') {
  		    encoding = byteOffset;
  		    byteOffset = 0;
  		  } else if (byteOffset > 0x7fffffff) {
  		    byteOffset = 0x7fffffff;
  		  } else if (byteOffset < -2147483648) {
  		    byteOffset = -2147483648;
  		  }
  		  byteOffset = +byteOffset; // Coerce to Number.
  		  if (numberIsNaN(byteOffset)) {
  		    // byteOffset: it it's undefined, null, NaN, "foo", etc, search whole buffer
  		    byteOffset = dir ? 0 : (buffer.length - 1);
  		  }

  		  // Normalize byteOffset: negative offsets start from the end of the buffer
  		  if (byteOffset < 0) byteOffset = buffer.length + byteOffset;
  		  if (byteOffset >= buffer.length) {
  		    if (dir) return -1
  		    else byteOffset = buffer.length - 1;
  		  } else if (byteOffset < 0) {
  		    if (dir) byteOffset = 0;
  		    else return -1
  		  }

  		  // Normalize val
  		  if (typeof val === 'string') {
  		    val = Buffer.from(val, encoding);
  		  }

  		  // Finally, search either indexOf (if dir is true) or lastIndexOf
  		  if (Buffer.isBuffer(val)) {
  		    // Special case: looking for empty string/buffer always fails
  		    if (val.length === 0) {
  		      return -1
  		    }
  		    return arrayIndexOf(buffer, val, byteOffset, encoding, dir)
  		  } else if (typeof val === 'number') {
  		    val = val & 0xFF; // Search for a byte value [0-255]
  		    if (typeof Uint8Array.prototype.indexOf === 'function') {
  		      if (dir) {
  		        return Uint8Array.prototype.indexOf.call(buffer, val, byteOffset)
  		      } else {
  		        return Uint8Array.prototype.lastIndexOf.call(buffer, val, byteOffset)
  		      }
  		    }
  		    return arrayIndexOf(buffer, [val], byteOffset, encoding, dir)
  		  }

  		  throw new TypeError('val must be string, number or Buffer')
  		}

  		function arrayIndexOf (arr, val, byteOffset, encoding, dir) {
  		  var indexSize = 1;
  		  var arrLength = arr.length;
  		  var valLength = val.length;

  		  if (encoding !== undefined) {
  		    encoding = String(encoding).toLowerCase();
  		    if (encoding === 'ucs2' || encoding === 'ucs-2' ||
  		        encoding === 'utf16le' || encoding === 'utf-16le') {
  		      if (arr.length < 2 || val.length < 2) {
  		        return -1
  		      }
  		      indexSize = 2;
  		      arrLength /= 2;
  		      valLength /= 2;
  		      byteOffset /= 2;
  		    }
  		  }

  		  function read (buf, i) {
  		    if (indexSize === 1) {
  		      return buf[i]
  		    } else {
  		      return buf.readUInt16BE(i * indexSize)
  		    }
  		  }

  		  var i;
  		  if (dir) {
  		    var foundIndex = -1;
  		    for (i = byteOffset; i < arrLength; i++) {
  		      if (read(arr, i) === read(val, foundIndex === -1 ? 0 : i - foundIndex)) {
  		        if (foundIndex === -1) foundIndex = i;
  		        if (i - foundIndex + 1 === valLength) return foundIndex * indexSize
  		      } else {
  		        if (foundIndex !== -1) i -= i - foundIndex;
  		        foundIndex = -1;
  		      }
  		    }
  		  } else {
  		    if (byteOffset + valLength > arrLength) byteOffset = arrLength - valLength;
  		    for (i = byteOffset; i >= 0; i--) {
  		      var found = true;
  		      for (var j = 0; j < valLength; j++) {
  		        if (read(arr, i + j) !== read(val, j)) {
  		          found = false;
  		          break
  		        }
  		      }
  		      if (found) return i
  		    }
  		  }

  		  return -1
  		}

  		Buffer.prototype.includes = function includes (val, byteOffset, encoding) {
  		  return this.indexOf(val, byteOffset, encoding) !== -1
  		};

  		Buffer.prototype.indexOf = function indexOf (val, byteOffset, encoding) {
  		  return bidirectionalIndexOf(this, val, byteOffset, encoding, true)
  		};

  		Buffer.prototype.lastIndexOf = function lastIndexOf (val, byteOffset, encoding) {
  		  return bidirectionalIndexOf(this, val, byteOffset, encoding, false)
  		};

  		function hexWrite (buf, string, offset, length) {
  		  offset = Number(offset) || 0;
  		  var remaining = buf.length - offset;
  		  if (!length) {
  		    length = remaining;
  		  } else {
  		    length = Number(length);
  		    if (length > remaining) {
  		      length = remaining;
  		    }
  		  }

  		  var strLen = string.length;

  		  if (length > strLen / 2) {
  		    length = strLen / 2;
  		  }
  		  for (var i = 0; i < length; ++i) {
  		    var parsed = parseInt(string.substr(i * 2, 2), 16);
  		    if (numberIsNaN(parsed)) return i
  		    buf[offset + i] = parsed;
  		  }
  		  return i
  		}

  		function utf8Write (buf, string, offset, length) {
  		  return blitBuffer(utf8ToBytes(string, buf.length - offset), buf, offset, length)
  		}

  		function asciiWrite (buf, string, offset, length) {
  		  return blitBuffer(asciiToBytes(string), buf, offset, length)
  		}

  		function base64Write (buf, string, offset, length) {
  		  return blitBuffer(base64ToBytes(string), buf, offset, length)
  		}

  		function ucs2Write (buf, string, offset, length) {
  		  return blitBuffer(utf16leToBytes(string, buf.length - offset), buf, offset, length)
  		}

  		Buffer.prototype.write = function write (string, offset, length, encoding) {
  		  // Buffer#write(string)
  		  if (offset === undefined) {
  		    encoding = 'utf8';
  		    length = this.length;
  		    offset = 0;
  		  // Buffer#write(string, encoding)
  		  } else if (length === undefined && typeof offset === 'string') {
  		    encoding = offset;
  		    length = this.length;
  		    offset = 0;
  		  // Buffer#write(string, offset[, length][, encoding])
  		  } else if (isFinite(offset)) {
  		    offset = offset >>> 0;
  		    if (isFinite(length)) {
  		      length = length >>> 0;
  		      if (encoding === undefined) encoding = 'utf8';
  		    } else {
  		      encoding = length;
  		      length = undefined;
  		    }
  		  } else {
  		    throw new Error(
  		      'Buffer.write(string, encoding, offset[, length]) is no longer supported'
  		    )
  		  }

  		  var remaining = this.length - offset;
  		  if (length === undefined || length > remaining) length = remaining;

  		  if ((string.length > 0 && (length < 0 || offset < 0)) || offset > this.length) {
  		    throw new RangeError('Attempt to write outside buffer bounds')
  		  }

  		  if (!encoding) encoding = 'utf8';

  		  var loweredCase = false;
  		  for (;;) {
  		    switch (encoding) {
  		      case 'hex':
  		        return hexWrite(this, string, offset, length)

  		      case 'utf8':
  		      case 'utf-8':
  		        return utf8Write(this, string, offset, length)

  		      case 'ascii':
  		      case 'latin1':
  		      case 'binary':
  		        return asciiWrite(this, string, offset, length)

  		      case 'base64':
  		        // Warning: maxLength not taken into account in base64Write
  		        return base64Write(this, string, offset, length)

  		      case 'ucs2':
  		      case 'ucs-2':
  		      case 'utf16le':
  		      case 'utf-16le':
  		        return ucs2Write(this, string, offset, length)

  		      default:
  		        if (loweredCase) throw new TypeError('Unknown encoding: ' + encoding)
  		        encoding = ('' + encoding).toLowerCase();
  		        loweredCase = true;
  		    }
  		  }
  		};

  		Buffer.prototype.toJSON = function toJSON () {
  		  return {
  		    type: 'Buffer',
  		    data: Array.prototype.slice.call(this._arr || this, 0)
  		  }
  		};

  		function base64Slice (buf, start, end) {
  		  if (start === 0 && end === buf.length) {
  		    return base64.fromByteArray(buf)
  		  } else {
  		    return base64.fromByteArray(buf.slice(start, end))
  		  }
  		}

  		function utf8Slice (buf, start, end) {
  		  end = Math.min(buf.length, end);
  		  var res = [];

  		  var i = start;
  		  while (i < end) {
  		    var firstByte = buf[i];
  		    var codePoint = null;
  		    var bytesPerSequence = (firstByte > 0xEF)
  		      ? 4
  		      : (firstByte > 0xDF)
  		          ? 3
  		          : (firstByte > 0xBF)
  		              ? 2
  		              : 1;

  		    if (i + bytesPerSequence <= end) {
  		      var secondByte, thirdByte, fourthByte, tempCodePoint;

  		      switch (bytesPerSequence) {
  		        case 1:
  		          if (firstByte < 0x80) {
  		            codePoint = firstByte;
  		          }
  		          break
  		        case 2:
  		          secondByte = buf[i + 1];
  		          if ((secondByte & 0xC0) === 0x80) {
  		            tempCodePoint = (firstByte & 0x1F) << 0x6 | (secondByte & 0x3F);
  		            if (tempCodePoint > 0x7F) {
  		              codePoint = tempCodePoint;
  		            }
  		          }
  		          break
  		        case 3:
  		          secondByte = buf[i + 1];
  		          thirdByte = buf[i + 2];
  		          if ((secondByte & 0xC0) === 0x80 && (thirdByte & 0xC0) === 0x80) {
  		            tempCodePoint = (firstByte & 0xF) << 0xC | (secondByte & 0x3F) << 0x6 | (thirdByte & 0x3F);
  		            if (tempCodePoint > 0x7FF && (tempCodePoint < 0xD800 || tempCodePoint > 0xDFFF)) {
  		              codePoint = tempCodePoint;
  		            }
  		          }
  		          break
  		        case 4:
  		          secondByte = buf[i + 1];
  		          thirdByte = buf[i + 2];
  		          fourthByte = buf[i + 3];
  		          if ((secondByte & 0xC0) === 0x80 && (thirdByte & 0xC0) === 0x80 && (fourthByte & 0xC0) === 0x80) {
  		            tempCodePoint = (firstByte & 0xF) << 0x12 | (secondByte & 0x3F) << 0xC | (thirdByte & 0x3F) << 0x6 | (fourthByte & 0x3F);
  		            if (tempCodePoint > 0xFFFF && tempCodePoint < 0x110000) {
  		              codePoint = tempCodePoint;
  		            }
  		          }
  		      }
  		    }

  		    if (codePoint === null) {
  		      // we did not generate a valid codePoint so insert a
  		      // replacement char (U+FFFD) and advance only 1 byte
  		      codePoint = 0xFFFD;
  		      bytesPerSequence = 1;
  		    } else if (codePoint > 0xFFFF) {
  		      // encode to utf16 (surrogate pair dance)
  		      codePoint -= 0x10000;
  		      res.push(codePoint >>> 10 & 0x3FF | 0xD800);
  		      codePoint = 0xDC00 | codePoint & 0x3FF;
  		    }

  		    res.push(codePoint);
  		    i += bytesPerSequence;
  		  }

  		  return decodeCodePointsArray(res)
  		}

  		// Based on http://stackoverflow.com/a/22747272/680742, the browser with
  		// the lowest limit is Chrome, with 0x10000 args.
  		// We go 1 magnitude less, for safety
  		var MAX_ARGUMENTS_LENGTH = 0x1000;

  		function decodeCodePointsArray (codePoints) {
  		  var len = codePoints.length;
  		  if (len <= MAX_ARGUMENTS_LENGTH) {
  		    return String.fromCharCode.apply(String, codePoints) // avoid extra slice()
  		  }

  		  // Decode in chunks to avoid "call stack size exceeded".
  		  var res = '';
  		  var i = 0;
  		  while (i < len) {
  		    res += String.fromCharCode.apply(
  		      String,
  		      codePoints.slice(i, i += MAX_ARGUMENTS_LENGTH)
  		    );
  		  }
  		  return res
  		}

  		function asciiSlice (buf, start, end) {
  		  var ret = '';
  		  end = Math.min(buf.length, end);

  		  for (var i = start; i < end; ++i) {
  		    ret += String.fromCharCode(buf[i] & 0x7F);
  		  }
  		  return ret
  		}

  		function latin1Slice (buf, start, end) {
  		  var ret = '';
  		  end = Math.min(buf.length, end);

  		  for (var i = start; i < end; ++i) {
  		    ret += String.fromCharCode(buf[i]);
  		  }
  		  return ret
  		}

  		function hexSlice (buf, start, end) {
  		  var len = buf.length;

  		  if (!start || start < 0) start = 0;
  		  if (!end || end < 0 || end > len) end = len;

  		  var out = '';
  		  for (var i = start; i < end; ++i) {
  		    out += hexSliceLookupTable[buf[i]];
  		  }
  		  return out
  		}

  		function utf16leSlice (buf, start, end) {
  		  var bytes = buf.slice(start, end);
  		  var res = '';
  		  // If bytes.length is odd, the last 8 bits must be ignored (same as node.js)
  		  for (var i = 0; i < bytes.length - 1; i += 2) {
  		    res += String.fromCharCode(bytes[i] + (bytes[i + 1] * 256));
  		  }
  		  return res
  		}

  		Buffer.prototype.slice = function slice (start, end) {
  		  var len = this.length;
  		  start = ~~start;
  		  end = end === undefined ? len : ~~end;

  		  if (start < 0) {
  		    start += len;
  		    if (start < 0) start = 0;
  		  } else if (start > len) {
  		    start = len;
  		  }

  		  if (end < 0) {
  		    end += len;
  		    if (end < 0) end = 0;
  		  } else if (end > len) {
  		    end = len;
  		  }

  		  if (end < start) end = start;

  		  var newBuf = this.subarray(start, end);
  		  // Return an augmented `Uint8Array` instance
  		  Object.setPrototypeOf(newBuf, Buffer.prototype);

  		  return newBuf
  		};

  		/*
  		 * Need to make sure that buffer isn't trying to write out of bounds.
  		 */
  		function checkOffset (offset, ext, length) {
  		  if ((offset % 1) !== 0 || offset < 0) throw new RangeError('offset is not uint')
  		  if (offset + ext > length) throw new RangeError('Trying to access beyond buffer length')
  		}

  		Buffer.prototype.readUintLE =
  		Buffer.prototype.readUIntLE = function readUIntLE (offset, byteLength, noAssert) {
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) checkOffset(offset, byteLength, this.length);

  		  var val = this[offset];
  		  var mul = 1;
  		  var i = 0;
  		  while (++i < byteLength && (mul *= 0x100)) {
  		    val += this[offset + i] * mul;
  		  }

  		  return val
  		};

  		Buffer.prototype.readUintBE =
  		Buffer.prototype.readUIntBE = function readUIntBE (offset, byteLength, noAssert) {
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) {
  		    checkOffset(offset, byteLength, this.length);
  		  }

  		  var val = this[offset + --byteLength];
  		  var mul = 1;
  		  while (byteLength > 0 && (mul *= 0x100)) {
  		    val += this[offset + --byteLength] * mul;
  		  }

  		  return val
  		};

  		Buffer.prototype.readUint8 =
  		Buffer.prototype.readUInt8 = function readUInt8 (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 1, this.length);
  		  return this[offset]
  		};

  		Buffer.prototype.readUint16LE =
  		Buffer.prototype.readUInt16LE = function readUInt16LE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 2, this.length);
  		  return this[offset] | (this[offset + 1] << 8)
  		};

  		Buffer.prototype.readUint16BE =
  		Buffer.prototype.readUInt16BE = function readUInt16BE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 2, this.length);
  		  return (this[offset] << 8) | this[offset + 1]
  		};

  		Buffer.prototype.readUint32LE =
  		Buffer.prototype.readUInt32LE = function readUInt32LE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);

  		  return ((this[offset]) |
  		      (this[offset + 1] << 8) |
  		      (this[offset + 2] << 16)) +
  		      (this[offset + 3] * 0x1000000)
  		};

  		Buffer.prototype.readUint32BE =
  		Buffer.prototype.readUInt32BE = function readUInt32BE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);

  		  return (this[offset] * 0x1000000) +
  		    ((this[offset + 1] << 16) |
  		    (this[offset + 2] << 8) |
  		    this[offset + 3])
  		};

  		Buffer.prototype.readIntLE = function readIntLE (offset, byteLength, noAssert) {
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) checkOffset(offset, byteLength, this.length);

  		  var val = this[offset];
  		  var mul = 1;
  		  var i = 0;
  		  while (++i < byteLength && (mul *= 0x100)) {
  		    val += this[offset + i] * mul;
  		  }
  		  mul *= 0x80;

  		  if (val >= mul) val -= Math.pow(2, 8 * byteLength);

  		  return val
  		};

  		Buffer.prototype.readIntBE = function readIntBE (offset, byteLength, noAssert) {
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) checkOffset(offset, byteLength, this.length);

  		  var i = byteLength;
  		  var mul = 1;
  		  var val = this[offset + --i];
  		  while (i > 0 && (mul *= 0x100)) {
  		    val += this[offset + --i] * mul;
  		  }
  		  mul *= 0x80;

  		  if (val >= mul) val -= Math.pow(2, 8 * byteLength);

  		  return val
  		};

  		Buffer.prototype.readInt8 = function readInt8 (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 1, this.length);
  		  if (!(this[offset] & 0x80)) return (this[offset])
  		  return ((0xff - this[offset] + 1) * -1)
  		};

  		Buffer.prototype.readInt16LE = function readInt16LE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 2, this.length);
  		  var val = this[offset] | (this[offset + 1] << 8);
  		  return (val & 0x8000) ? val | 0xFFFF0000 : val
  		};

  		Buffer.prototype.readInt16BE = function readInt16BE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 2, this.length);
  		  var val = this[offset + 1] | (this[offset] << 8);
  		  return (val & 0x8000) ? val | 0xFFFF0000 : val
  		};

  		Buffer.prototype.readInt32LE = function readInt32LE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);

  		  return (this[offset]) |
  		    (this[offset + 1] << 8) |
  		    (this[offset + 2] << 16) |
  		    (this[offset + 3] << 24)
  		};

  		Buffer.prototype.readInt32BE = function readInt32BE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);

  		  return (this[offset] << 24) |
  		    (this[offset + 1] << 16) |
  		    (this[offset + 2] << 8) |
  		    (this[offset + 3])
  		};

  		Buffer.prototype.readFloatLE = function readFloatLE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);
  		  return ieee754.read(this, offset, true, 23, 4)
  		};

  		Buffer.prototype.readFloatBE = function readFloatBE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 4, this.length);
  		  return ieee754.read(this, offset, false, 23, 4)
  		};

  		Buffer.prototype.readDoubleLE = function readDoubleLE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 8, this.length);
  		  return ieee754.read(this, offset, true, 52, 8)
  		};

  		Buffer.prototype.readDoubleBE = function readDoubleBE (offset, noAssert) {
  		  offset = offset >>> 0;
  		  if (!noAssert) checkOffset(offset, 8, this.length);
  		  return ieee754.read(this, offset, false, 52, 8)
  		};

  		function checkInt (buf, value, offset, ext, max, min) {
  		  if (!Buffer.isBuffer(buf)) throw new TypeError('"buffer" argument must be a Buffer instance')
  		  if (value > max || value < min) throw new RangeError('"value" argument is out of bounds')
  		  if (offset + ext > buf.length) throw new RangeError('Index out of range')
  		}

  		Buffer.prototype.writeUintLE =
  		Buffer.prototype.writeUIntLE = function writeUIntLE (value, offset, byteLength, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) {
  		    var maxBytes = Math.pow(2, 8 * byteLength) - 1;
  		    checkInt(this, value, offset, byteLength, maxBytes, 0);
  		  }

  		  var mul = 1;
  		  var i = 0;
  		  this[offset] = value & 0xFF;
  		  while (++i < byteLength && (mul *= 0x100)) {
  		    this[offset + i] = (value / mul) & 0xFF;
  		  }

  		  return offset + byteLength
  		};

  		Buffer.prototype.writeUintBE =
  		Buffer.prototype.writeUIntBE = function writeUIntBE (value, offset, byteLength, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  byteLength = byteLength >>> 0;
  		  if (!noAssert) {
  		    var maxBytes = Math.pow(2, 8 * byteLength) - 1;
  		    checkInt(this, value, offset, byteLength, maxBytes, 0);
  		  }

  		  var i = byteLength - 1;
  		  var mul = 1;
  		  this[offset + i] = value & 0xFF;
  		  while (--i >= 0 && (mul *= 0x100)) {
  		    this[offset + i] = (value / mul) & 0xFF;
  		  }

  		  return offset + byteLength
  		};

  		Buffer.prototype.writeUint8 =
  		Buffer.prototype.writeUInt8 = function writeUInt8 (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 1, 0xff, 0);
  		  this[offset] = (value & 0xff);
  		  return offset + 1
  		};

  		Buffer.prototype.writeUint16LE =
  		Buffer.prototype.writeUInt16LE = function writeUInt16LE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 2, 0xffff, 0);
  		  this[offset] = (value & 0xff);
  		  this[offset + 1] = (value >>> 8);
  		  return offset + 2
  		};

  		Buffer.prototype.writeUint16BE =
  		Buffer.prototype.writeUInt16BE = function writeUInt16BE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 2, 0xffff, 0);
  		  this[offset] = (value >>> 8);
  		  this[offset + 1] = (value & 0xff);
  		  return offset + 2
  		};

  		Buffer.prototype.writeUint32LE =
  		Buffer.prototype.writeUInt32LE = function writeUInt32LE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 4, 0xffffffff, 0);
  		  this[offset + 3] = (value >>> 24);
  		  this[offset + 2] = (value >>> 16);
  		  this[offset + 1] = (value >>> 8);
  		  this[offset] = (value & 0xff);
  		  return offset + 4
  		};

  		Buffer.prototype.writeUint32BE =
  		Buffer.prototype.writeUInt32BE = function writeUInt32BE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 4, 0xffffffff, 0);
  		  this[offset] = (value >>> 24);
  		  this[offset + 1] = (value >>> 16);
  		  this[offset + 2] = (value >>> 8);
  		  this[offset + 3] = (value & 0xff);
  		  return offset + 4
  		};

  		Buffer.prototype.writeIntLE = function writeIntLE (value, offset, byteLength, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) {
  		    var limit = Math.pow(2, (8 * byteLength) - 1);

  		    checkInt(this, value, offset, byteLength, limit - 1, -limit);
  		  }

  		  var i = 0;
  		  var mul = 1;
  		  var sub = 0;
  		  this[offset] = value & 0xFF;
  		  while (++i < byteLength && (mul *= 0x100)) {
  		    if (value < 0 && sub === 0 && this[offset + i - 1] !== 0) {
  		      sub = 1;
  		    }
  		    this[offset + i] = ((value / mul) >> 0) - sub & 0xFF;
  		  }

  		  return offset + byteLength
  		};

  		Buffer.prototype.writeIntBE = function writeIntBE (value, offset, byteLength, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) {
  		    var limit = Math.pow(2, (8 * byteLength) - 1);

  		    checkInt(this, value, offset, byteLength, limit - 1, -limit);
  		  }

  		  var i = byteLength - 1;
  		  var mul = 1;
  		  var sub = 0;
  		  this[offset + i] = value & 0xFF;
  		  while (--i >= 0 && (mul *= 0x100)) {
  		    if (value < 0 && sub === 0 && this[offset + i + 1] !== 0) {
  		      sub = 1;
  		    }
  		    this[offset + i] = ((value / mul) >> 0) - sub & 0xFF;
  		  }

  		  return offset + byteLength
  		};

  		Buffer.prototype.writeInt8 = function writeInt8 (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 1, 0x7f, -128);
  		  if (value < 0) value = 0xff + value + 1;
  		  this[offset] = (value & 0xff);
  		  return offset + 1
  		};

  		Buffer.prototype.writeInt16LE = function writeInt16LE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 2, 0x7fff, -32768);
  		  this[offset] = (value & 0xff);
  		  this[offset + 1] = (value >>> 8);
  		  return offset + 2
  		};

  		Buffer.prototype.writeInt16BE = function writeInt16BE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 2, 0x7fff, -32768);
  		  this[offset] = (value >>> 8);
  		  this[offset + 1] = (value & 0xff);
  		  return offset + 2
  		};

  		Buffer.prototype.writeInt32LE = function writeInt32LE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 4, 0x7fffffff, -2147483648);
  		  this[offset] = (value & 0xff);
  		  this[offset + 1] = (value >>> 8);
  		  this[offset + 2] = (value >>> 16);
  		  this[offset + 3] = (value >>> 24);
  		  return offset + 4
  		};

  		Buffer.prototype.writeInt32BE = function writeInt32BE (value, offset, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) checkInt(this, value, offset, 4, 0x7fffffff, -2147483648);
  		  if (value < 0) value = 0xffffffff + value + 1;
  		  this[offset] = (value >>> 24);
  		  this[offset + 1] = (value >>> 16);
  		  this[offset + 2] = (value >>> 8);
  		  this[offset + 3] = (value & 0xff);
  		  return offset + 4
  		};

  		function checkIEEE754 (buf, value, offset, ext, max, min) {
  		  if (offset + ext > buf.length) throw new RangeError('Index out of range')
  		  if (offset < 0) throw new RangeError('Index out of range')
  		}

  		function writeFloat (buf, value, offset, littleEndian, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) {
  		    checkIEEE754(buf, value, offset, 4);
  		  }
  		  ieee754.write(buf, value, offset, littleEndian, 23, 4);
  		  return offset + 4
  		}

  		Buffer.prototype.writeFloatLE = function writeFloatLE (value, offset, noAssert) {
  		  return writeFloat(this, value, offset, true, noAssert)
  		};

  		Buffer.prototype.writeFloatBE = function writeFloatBE (value, offset, noAssert) {
  		  return writeFloat(this, value, offset, false, noAssert)
  		};

  		function writeDouble (buf, value, offset, littleEndian, noAssert) {
  		  value = +value;
  		  offset = offset >>> 0;
  		  if (!noAssert) {
  		    checkIEEE754(buf, value, offset, 8);
  		  }
  		  ieee754.write(buf, value, offset, littleEndian, 52, 8);
  		  return offset + 8
  		}

  		Buffer.prototype.writeDoubleLE = function writeDoubleLE (value, offset, noAssert) {
  		  return writeDouble(this, value, offset, true, noAssert)
  		};

  		Buffer.prototype.writeDoubleBE = function writeDoubleBE (value, offset, noAssert) {
  		  return writeDouble(this, value, offset, false, noAssert)
  		};

  		// copy(targetBuffer, targetStart=0, sourceStart=0, sourceEnd=buffer.length)
  		Buffer.prototype.copy = function copy (target, targetStart, start, end) {
  		  if (!Buffer.isBuffer(target)) throw new TypeError('argument should be a Buffer')
  		  if (!start) start = 0;
  		  if (!end && end !== 0) end = this.length;
  		  if (targetStart >= target.length) targetStart = target.length;
  		  if (!targetStart) targetStart = 0;
  		  if (end > 0 && end < start) end = start;

  		  // Copy 0 bytes; we're done
  		  if (end === start) return 0
  		  if (target.length === 0 || this.length === 0) return 0

  		  // Fatal error conditions
  		  if (targetStart < 0) {
  		    throw new RangeError('targetStart out of bounds')
  		  }
  		  if (start < 0 || start >= this.length) throw new RangeError('Index out of range')
  		  if (end < 0) throw new RangeError('sourceEnd out of bounds')

  		  // Are we oob?
  		  if (end > this.length) end = this.length;
  		  if (target.length - targetStart < end - start) {
  		    end = target.length - targetStart + start;
  		  }

  		  var len = end - start;

  		  if (this === target && typeof Uint8Array.prototype.copyWithin === 'function') {
  		    // Use built-in when available, missing from IE11
  		    this.copyWithin(targetStart, start, end);
  		  } else {
  		    Uint8Array.prototype.set.call(
  		      target,
  		      this.subarray(start, end),
  		      targetStart
  		    );
  		  }

  		  return len
  		};

  		// Usage:
  		//    buffer.fill(number[, offset[, end]])
  		//    buffer.fill(buffer[, offset[, end]])
  		//    buffer.fill(string[, offset[, end]][, encoding])
  		Buffer.prototype.fill = function fill (val, start, end, encoding) {
  		  // Handle string cases:
  		  if (typeof val === 'string') {
  		    if (typeof start === 'string') {
  		      encoding = start;
  		      start = 0;
  		      end = this.length;
  		    } else if (typeof end === 'string') {
  		      encoding = end;
  		      end = this.length;
  		    }
  		    if (encoding !== undefined && typeof encoding !== 'string') {
  		      throw new TypeError('encoding must be a string')
  		    }
  		    if (typeof encoding === 'string' && !Buffer.isEncoding(encoding)) {
  		      throw new TypeError('Unknown encoding: ' + encoding)
  		    }
  		    if (val.length === 1) {
  		      var code = val.charCodeAt(0);
  		      if ((encoding === 'utf8' && code < 128) ||
  		          encoding === 'latin1') {
  		        // Fast path: If `val` fits into a single byte, use that numeric value.
  		        val = code;
  		      }
  		    }
  		  } else if (typeof val === 'number') {
  		    val = val & 255;
  		  } else if (typeof val === 'boolean') {
  		    val = Number(val);
  		  }

  		  // Invalid ranges are not set to a default, so can range check early.
  		  if (start < 0 || this.length < start || this.length < end) {
  		    throw new RangeError('Out of range index')
  		  }

  		  if (end <= start) {
  		    return this
  		  }

  		  start = start >>> 0;
  		  end = end === undefined ? this.length : end >>> 0;

  		  if (!val) val = 0;

  		  var i;
  		  if (typeof val === 'number') {
  		    for (i = start; i < end; ++i) {
  		      this[i] = val;
  		    }
  		  } else {
  		    var bytes = Buffer.isBuffer(val)
  		      ? val
  		      : Buffer.from(val, encoding);
  		    var len = bytes.length;
  		    if (len === 0) {
  		      throw new TypeError('The value "' + val +
  		        '" is invalid for argument "value"')
  		    }
  		    for (i = 0; i < end - start; ++i) {
  		      this[i + start] = bytes[i % len];
  		    }
  		  }

  		  return this
  		};

  		// HELPER FUNCTIONS
  		// ================

  		var INVALID_BASE64_RE = /[^+/0-9A-Za-z-_]/g;

  		function base64clean (str) {
  		  // Node takes equal signs as end of the Base64 encoding
  		  str = str.split('=')[0];
  		  // Node strips out invalid characters like \n and \t from the string, base64-js does not
  		  str = str.trim().replace(INVALID_BASE64_RE, '');
  		  // Node converts strings with length < 2 to ''
  		  if (str.length < 2) return ''
  		  // Node allows for non-padded base64 strings (missing trailing ===), base64-js does not
  		  while (str.length % 4 !== 0) {
  		    str = str + '=';
  		  }
  		  return str
  		}

  		function utf8ToBytes (string, units) {
  		  units = units || Infinity;
  		  var codePoint;
  		  var length = string.length;
  		  var leadSurrogate = null;
  		  var bytes = [];

  		  for (var i = 0; i < length; ++i) {
  		    codePoint = string.charCodeAt(i);

  		    // is surrogate component
  		    if (codePoint > 0xD7FF && codePoint < 0xE000) {
  		      // last char was a lead
  		      if (!leadSurrogate) {
  		        // no lead yet
  		        if (codePoint > 0xDBFF) {
  		          // unexpected trail
  		          if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD);
  		          continue
  		        } else if (i + 1 === length) {
  		          // unpaired lead
  		          if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD);
  		          continue
  		        }

  		        // valid lead
  		        leadSurrogate = codePoint;

  		        continue
  		      }

  		      // 2 leads in a row
  		      if (codePoint < 0xDC00) {
  		        if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD);
  		        leadSurrogate = codePoint;
  		        continue
  		      }

  		      // valid surrogate pair
  		      codePoint = (leadSurrogate - 0xD800 << 10 | codePoint - 0xDC00) + 0x10000;
  		    } else if (leadSurrogate) {
  		      // valid bmp char, but last char was a lead
  		      if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD);
  		    }

  		    leadSurrogate = null;

  		    // encode utf8
  		    if (codePoint < 0x80) {
  		      if ((units -= 1) < 0) break
  		      bytes.push(codePoint);
  		    } else if (codePoint < 0x800) {
  		      if ((units -= 2) < 0) break
  		      bytes.push(
  		        codePoint >> 0x6 | 0xC0,
  		        codePoint & 0x3F | 0x80
  		      );
  		    } else if (codePoint < 0x10000) {
  		      if ((units -= 3) < 0) break
  		      bytes.push(
  		        codePoint >> 0xC | 0xE0,
  		        codePoint >> 0x6 & 0x3F | 0x80,
  		        codePoint & 0x3F | 0x80
  		      );
  		    } else if (codePoint < 0x110000) {
  		      if ((units -= 4) < 0) break
  		      bytes.push(
  		        codePoint >> 0x12 | 0xF0,
  		        codePoint >> 0xC & 0x3F | 0x80,
  		        codePoint >> 0x6 & 0x3F | 0x80,
  		        codePoint & 0x3F | 0x80
  		      );
  		    } else {
  		      throw new Error('Invalid code point')
  		    }
  		  }

  		  return bytes
  		}

  		function asciiToBytes (str) {
  		  var byteArray = [];
  		  for (var i = 0; i < str.length; ++i) {
  		    // Node's code seems to be doing this and not & 0x7F..
  		    byteArray.push(str.charCodeAt(i) & 0xFF);
  		  }
  		  return byteArray
  		}

  		function utf16leToBytes (str, units) {
  		  var c, hi, lo;
  		  var byteArray = [];
  		  for (var i = 0; i < str.length; ++i) {
  		    if ((units -= 2) < 0) break

  		    c = str.charCodeAt(i);
  		    hi = c >> 8;
  		    lo = c % 256;
  		    byteArray.push(lo);
  		    byteArray.push(hi);
  		  }

  		  return byteArray
  		}

  		function base64ToBytes (str) {
  		  return base64.toByteArray(base64clean(str))
  		}

  		function blitBuffer (src, dst, offset, length) {
  		  for (var i = 0; i < length; ++i) {
  		    if ((i + offset >= dst.length) || (i >= src.length)) break
  		    dst[i + offset] = src[i];
  		  }
  		  return i
  		}

  		// ArrayBuffer or Uint8Array objects from other contexts (i.e. iframes) do not pass
  		// the `instanceof` check but they should be treated as of that type.
  		// See: https://github.com/feross/buffer/issues/166
  		function isInstance (obj, type) {
  		  return obj instanceof type ||
  		    (obj != null && obj.constructor != null && obj.constructor.name != null &&
  		      obj.constructor.name === type.name)
  		}
  		function numberIsNaN (obj) {
  		  // For IE11 support
  		  return obj !== obj // eslint-disable-line no-self-compare
  		}

  		// Create lookup table for `toString('hex')`
  		// See: https://github.com/feross/buffer/issues/219
  		var hexSliceLookupTable = (function () {
  		  var alphabet = '0123456789abcdef';
  		  var table = new Array(256);
  		  for (var i = 0; i < 16; ++i) {
  		    var i16 = i * 16;
  		    for (var j = 0; j < 16; ++j) {
  		      table[i16 + j] = alphabet[i] + alphabet[j];
  		    }
  		  }
  		  return table
  		})(); 
  	} (buffer));
  	return buffer;
  }

  var bufferExports = requireBuffer();

  var browser = {exports: {}};

  var hasRequiredBrowser;

  function requireBrowser () {
  	if (hasRequiredBrowser) return browser.exports;
  	hasRequiredBrowser = 1;
  	// shim for using process in browser
  	var process = browser.exports = {};

  	// cached from whatever global is present so that test runners that stub it
  	// don't break things.  But we need to wrap it in a try catch in case it is
  	// wrapped in strict mode code which doesn't define any globals.  It's inside a
  	// function because try/catches deoptimize in certain engines.

  	var cachedSetTimeout;
  	var cachedClearTimeout;

  	function defaultSetTimout() {
  	    throw new Error('setTimeout has not been defined');
  	}
  	function defaultClearTimeout () {
  	    throw new Error('clearTimeout has not been defined');
  	}
  	(function () {
  	    try {
  	        if (typeof setTimeout === 'function') {
  	            cachedSetTimeout = setTimeout;
  	        } else {
  	            cachedSetTimeout = defaultSetTimout;
  	        }
  	    } catch (e) {
  	        cachedSetTimeout = defaultSetTimout;
  	    }
  	    try {
  	        if (typeof clearTimeout === 'function') {
  	            cachedClearTimeout = clearTimeout;
  	        } else {
  	            cachedClearTimeout = defaultClearTimeout;
  	        }
  	    } catch (e) {
  	        cachedClearTimeout = defaultClearTimeout;
  	    }
  	} ());
  	function runTimeout(fun) {
  	    if (cachedSetTimeout === setTimeout) {
  	        //normal enviroments in sane situations
  	        return setTimeout(fun, 0);
  	    }
  	    // if setTimeout wasn't available but was latter defined
  	    if ((cachedSetTimeout === defaultSetTimout || !cachedSetTimeout) && setTimeout) {
  	        cachedSetTimeout = setTimeout;
  	        return setTimeout(fun, 0);
  	    }
  	    try {
  	        // when when somebody has screwed with setTimeout but no I.E. maddness
  	        return cachedSetTimeout(fun, 0);
  	    } catch(e){
  	        try {
  	            // When we are in I.E. but the script has been evaled so I.E. doesn't trust the global object when called normally
  	            return cachedSetTimeout.call(null, fun, 0);
  	        } catch(e){
  	            // same as above but when it's a version of I.E. that must have the global object for 'this', hopfully our context correct otherwise it will throw a global error
  	            return cachedSetTimeout.call(this, fun, 0);
  	        }
  	    }


  	}
  	function runClearTimeout(marker) {
  	    if (cachedClearTimeout === clearTimeout) {
  	        //normal enviroments in sane situations
  	        return clearTimeout(marker);
  	    }
  	    // if clearTimeout wasn't available but was latter defined
  	    if ((cachedClearTimeout === defaultClearTimeout || !cachedClearTimeout) && clearTimeout) {
  	        cachedClearTimeout = clearTimeout;
  	        return clearTimeout(marker);
  	    }
  	    try {
  	        // when when somebody has screwed with setTimeout but no I.E. maddness
  	        return cachedClearTimeout(marker);
  	    } catch (e){
  	        try {
  	            // When we are in I.E. but the script has been evaled so I.E. doesn't  trust the global object when called normally
  	            return cachedClearTimeout.call(null, marker);
  	        } catch (e){
  	            // same as above but when it's a version of I.E. that must have the global object for 'this', hopfully our context correct otherwise it will throw a global error.
  	            // Some versions of I.E. have different rules for clearTimeout vs setTimeout
  	            return cachedClearTimeout.call(this, marker);
  	        }
  	    }



  	}
  	var queue = [];
  	var draining = false;
  	var currentQueue;
  	var queueIndex = -1;

  	function cleanUpNextTick() {
  	    if (!draining || !currentQueue) {
  	        return;
  	    }
  	    draining = false;
  	    if (currentQueue.length) {
  	        queue = currentQueue.concat(queue);
  	    } else {
  	        queueIndex = -1;
  	    }
  	    if (queue.length) {
  	        drainQueue();
  	    }
  	}

  	function drainQueue() {
  	    if (draining) {
  	        return;
  	    }
  	    var timeout = runTimeout(cleanUpNextTick);
  	    draining = true;

  	    var len = queue.length;
  	    while(len) {
  	        currentQueue = queue;
  	        queue = [];
  	        while (++queueIndex < len) {
  	            if (currentQueue) {
  	                currentQueue[queueIndex].run();
  	            }
  	        }
  	        queueIndex = -1;
  	        len = queue.length;
  	    }
  	    currentQueue = null;
  	    draining = false;
  	    runClearTimeout(timeout);
  	}

  	process.nextTick = function (fun) {
  	    var args = new Array(arguments.length - 1);
  	    if (arguments.length > 1) {
  	        for (var i = 1; i < arguments.length; i++) {
  	            args[i - 1] = arguments[i];
  	        }
  	    }
  	    queue.push(new Item(fun, args));
  	    if (queue.length === 1 && !draining) {
  	        runTimeout(drainQueue);
  	    }
  	};

  	// v8 likes predictible objects
  	function Item(fun, array) {
  	    this.fun = fun;
  	    this.array = array;
  	}
  	Item.prototype.run = function () {
  	    this.fun.apply(null, this.array);
  	};
  	process.title = 'browser';
  	process.browser = true;
  	process.env = {};
  	process.argv = [];
  	process.version = ''; // empty string to avoid regexp issues
  	process.versions = {};

  	function noop() {}

  	process.on = noop;
  	process.addListener = noop;
  	process.once = noop;
  	process.off = noop;
  	process.removeListener = noop;
  	process.removeAllListeners = noop;
  	process.emit = noop;
  	process.prependListener = noop;
  	process.prependOnceListener = noop;

  	process.listeners = function (name) { return [] };

  	process.binding = function (name) {
  	    throw new Error('process.binding is not supported');
  	};

  	process.cwd = function () { return '/' };
  	process.chdir = function (dir) {
  	    throw new Error('process.chdir is not supported');
  	};
  	process.umask = function() { return 0; };
  	return browser.exports;
  }

  var browserExports = requireBrowser();
  const process = /*@__PURE__*/getDefaultExportFromCjs(browserExports);

  const VERSION = '0.36.3'; // x-release-please-version

  /**
   * Disclaimer: modules in _shims aren't intended to be imported by SDK users.
   */
  if (!kind) setShims(getRuntime(), { auto: true });

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class AnthropicError extends Error {
  }
  class APIError extends AnthropicError {
      constructor(status, error, message, headers) {
          super(`${APIError.makeMessage(status, error, message)}`);
          this.status = status;
          this.headers = headers;
          this.request_id = headers?.['request-id'];
          this.error = error;
      }
      static makeMessage(status, error, message) {
          const msg = error?.message ?
              typeof error.message === 'string' ?
                  error.message
                  : JSON.stringify(error.message)
              : error ? JSON.stringify(error)
                  : message;
          if (status && msg) {
              return `${status} ${msg}`;
          }
          if (status) {
              return `${status} status code (no body)`;
          }
          if (msg) {
              return msg;
          }
          return '(no status code or body)';
      }
      static generate(status, errorResponse, message, headers) {
          if (!status || !headers) {
              return new APIConnectionError({ message, cause: castToError(errorResponse) });
          }
          const error = errorResponse;
          if (status === 400) {
              return new BadRequestError(status, error, message, headers);
          }
          if (status === 401) {
              return new AuthenticationError(status, error, message, headers);
          }
          if (status === 403) {
              return new PermissionDeniedError(status, error, message, headers);
          }
          if (status === 404) {
              return new NotFoundError(status, error, message, headers);
          }
          if (status === 409) {
              return new ConflictError(status, error, message, headers);
          }
          if (status === 422) {
              return new UnprocessableEntityError(status, error, message, headers);
          }
          if (status === 429) {
              return new RateLimitError(status, error, message, headers);
          }
          if (status >= 500) {
              return new InternalServerError(status, error, message, headers);
          }
          return new APIError(status, error, message, headers);
      }
  }
  class APIUserAbortError extends APIError {
      constructor({ message } = {}) {
          super(undefined, undefined, message || 'Request was aborted.', undefined);
      }
  }
  class APIConnectionError extends APIError {
      constructor({ message, cause }) {
          super(undefined, undefined, message || 'Connection error.', undefined);
          // in some environments the 'cause' property is already declared
          // @ts-ignore
          if (cause)
              this.cause = cause;
      }
  }
  class APIConnectionTimeoutError extends APIConnectionError {
      constructor({ message } = {}) {
          super({ message: message ?? 'Request timed out.' });
      }
  }
  class BadRequestError extends APIError {
  }
  class AuthenticationError extends APIError {
  }
  class PermissionDeniedError extends APIError {
  }
  class NotFoundError extends APIError {
  }
  class ConflictError extends APIError {
  }
  class UnprocessableEntityError extends APIError {
  }
  class RateLimitError extends APIError {
  }
  class InternalServerError extends APIError {
  }

  /**
   * A re-implementation of httpx's `LineDecoder` in Python that handles incrementally
   * reading lines from text.
   *
   * https://github.com/encode/httpx/blob/920333ea98118e9cf617f246905d7b202510941c/httpx/_decoders.py#L258
   */
  class LineDecoder {
      constructor() {
          this.buffer = [];
          this.trailingCR = false;
      }
      decode(chunk) {
          let text = this.decodeText(chunk);
          if (this.trailingCR) {
              text = '\r' + text;
              this.trailingCR = false;
          }
          if (text.endsWith('\r')) {
              this.trailingCR = true;
              text = text.slice(0, -1);
          }
          if (!text) {
              return [];
          }
          const trailingNewline = LineDecoder.NEWLINE_CHARS.has(text[text.length - 1] || '');
          let lines = text.split(LineDecoder.NEWLINE_REGEXP);
          // if there is a trailing new line then the last entry will be an empty
          // string which we don't care about
          if (trailingNewline) {
              lines.pop();
          }
          if (lines.length === 1 && !trailingNewline) {
              this.buffer.push(lines[0]);
              return [];
          }
          if (this.buffer.length > 0) {
              lines = [this.buffer.join('') + lines[0], ...lines.slice(1)];
              this.buffer = [];
          }
          if (!trailingNewline) {
              this.buffer = [lines.pop() || ''];
          }
          return lines;
      }
      decodeText(bytes) {
          if (bytes == null)
              return '';
          if (typeof bytes === 'string')
              return bytes;
          // Node:
          if (typeof bufferExports.Buffer !== 'undefined') {
              if (bytes instanceof bufferExports.Buffer) {
                  return bytes.toString();
              }
              if (bytes instanceof Uint8Array) {
                  return bufferExports.Buffer.from(bytes).toString();
              }
              throw new AnthropicError(`Unexpected: received non-Uint8Array (${bytes.constructor.name}) stream chunk in an environment with a global "Buffer" defined, which this library assumes to be Node. Please report this error.`);
          }
          // Browser
          if (typeof TextDecoder !== 'undefined') {
              if (bytes instanceof Uint8Array || bytes instanceof ArrayBuffer) {
                  this.textDecoder ?? (this.textDecoder = new TextDecoder('utf8'));
                  return this.textDecoder.decode(bytes);
              }
              throw new AnthropicError(`Unexpected: received non-Uint8Array/ArrayBuffer (${bytes.constructor.name}) in a web platform. Please report this error.`);
          }
          throw new AnthropicError(`Unexpected: neither Buffer nor TextDecoder are available as globals. Please report this error.`);
      }
      flush() {
          if (!this.buffer.length && !this.trailingCR) {
              return [];
          }
          const lines = [this.buffer.join('')];
          this.buffer = [];
          this.trailingCR = false;
          return lines;
      }
  }
  // prettier-ignore
  LineDecoder.NEWLINE_CHARS = new Set(['\n', '\r']);
  LineDecoder.NEWLINE_REGEXP = /\r\n|[\n\r]/g;

  /**
   * Most browsers don't yet have async iterable support for ReadableStream,
   * and Node has a very different way of reading bytes from its "ReadableStream".
   *
   * This polyfill was pulled from https://github.com/MattiasBuelens/web-streams-polyfill/pull/122#issuecomment-1627354490
   */
  function ReadableStreamToAsyncIterable(stream) {
      if (stream[Symbol.asyncIterator])
          return stream;
      const reader = stream.getReader();
      return {
          async next() {
              try {
                  const result = await reader.read();
                  if (result?.done)
                      reader.releaseLock(); // release lock when stream becomes closed
                  return result;
              }
              catch (e) {
                  reader.releaseLock(); // release lock when stream becomes errored
                  throw e;
              }
          },
          async return() {
              const cancelPromise = reader.cancel();
              reader.releaseLock();
              await cancelPromise;
              return { done: true, value: undefined };
          },
          [Symbol.asyncIterator]() {
              return this;
          },
      };
  }

  class Stream {
      constructor(iterator, controller) {
          this.iterator = iterator;
          this.controller = controller;
      }
      static fromSSEResponse(response, controller) {
          let consumed = false;
          async function* iterator() {
              if (consumed) {
                  throw new Error('Cannot iterate over a consumed stream, use `.tee()` to split the stream.');
              }
              consumed = true;
              let done = false;
              try {
                  for await (const sse of _iterSSEMessages(response, controller)) {
                      if (sse.event === 'completion') {
                          try {
                              yield JSON.parse(sse.data);
                          }
                          catch (e) {
                              console.error(`Could not parse message into JSON:`, sse.data);
                              console.error(`From chunk:`, sse.raw);
                              throw e;
                          }
                      }
                      if (sse.event === 'message_start' ||
                          sse.event === 'message_delta' ||
                          sse.event === 'message_stop' ||
                          sse.event === 'content_block_start' ||
                          sse.event === 'content_block_delta' ||
                          sse.event === 'content_block_stop') {
                          try {
                              yield JSON.parse(sse.data);
                          }
                          catch (e) {
                              console.error(`Could not parse message into JSON:`, sse.data);
                              console.error(`From chunk:`, sse.raw);
                              throw e;
                          }
                      }
                      if (sse.event === 'ping') {
                          continue;
                      }
                      if (sse.event === 'error') {
                          throw APIError.generate(undefined, `SSE Error: ${sse.data}`, sse.data, createResponseHeaders(response.headers));
                      }
                  }
                  done = true;
              }
              catch (e) {
                  // If the user calls `stream.controller.abort()`, we should exit without throwing.
                  if (e instanceof Error && e.name === 'AbortError')
                      return;
                  throw e;
              }
              finally {
                  // If the user `break`s, abort the ongoing request.
                  if (!done)
                      controller.abort();
              }
          }
          return new Stream(iterator, controller);
      }
      /**
       * Generates a Stream from a newline-separated ReadableStream
       * where each item is a JSON value.
       */
      static fromReadableStream(readableStream, controller) {
          let consumed = false;
          async function* iterLines() {
              const lineDecoder = new LineDecoder();
              const iter = ReadableStreamToAsyncIterable(readableStream);
              for await (const chunk of iter) {
                  for (const line of lineDecoder.decode(chunk)) {
                      yield line;
                  }
              }
              for (const line of lineDecoder.flush()) {
                  yield line;
              }
          }
          async function* iterator() {
              if (consumed) {
                  throw new Error('Cannot iterate over a consumed stream, use `.tee()` to split the stream.');
              }
              consumed = true;
              let done = false;
              try {
                  for await (const line of iterLines()) {
                      if (done)
                          continue;
                      if (line)
                          yield JSON.parse(line);
                  }
                  done = true;
              }
              catch (e) {
                  // If the user calls `stream.controller.abort()`, we should exit without throwing.
                  if (e instanceof Error && e.name === 'AbortError')
                      return;
                  throw e;
              }
              finally {
                  // If the user `break`s, abort the ongoing request.
                  if (!done)
                      controller.abort();
              }
          }
          return new Stream(iterator, controller);
      }
      [Symbol.asyncIterator]() {
          return this.iterator();
      }
      /**
       * Splits the stream into two streams which can be
       * independently read from at different speeds.
       */
      tee() {
          const left = [];
          const right = [];
          const iterator = this.iterator();
          const teeIterator = (queue) => {
              return {
                  next: () => {
                      if (queue.length === 0) {
                          const result = iterator.next();
                          left.push(result);
                          right.push(result);
                      }
                      return queue.shift();
                  },
              };
          };
          return [
              new Stream(() => teeIterator(left), this.controller),
              new Stream(() => teeIterator(right), this.controller),
          ];
      }
      /**
       * Converts this stream to a newline-separated ReadableStream of
       * JSON stringified values in the stream
       * which can be turned back into a Stream with `Stream.fromReadableStream()`.
       */
      toReadableStream() {
          const self = this;
          let iter;
          const encoder = new TextEncoder();
          return new ReadableStream$1({
              async start() {
                  iter = self[Symbol.asyncIterator]();
              },
              async pull(ctrl) {
                  try {
                      const { value, done } = await iter.next();
                      if (done)
                          return ctrl.close();
                      const bytes = encoder.encode(JSON.stringify(value) + '\n');
                      ctrl.enqueue(bytes);
                  }
                  catch (err) {
                      ctrl.error(err);
                  }
              },
              async cancel() {
                  await iter.return?.();
              },
          });
      }
  }
  async function* _iterSSEMessages(response, controller) {
      if (!response.body) {
          controller.abort();
          throw new AnthropicError(`Attempted to iterate over a response with no body`);
      }
      const sseDecoder = new SSEDecoder();
      const lineDecoder = new LineDecoder();
      const iter = ReadableStreamToAsyncIterable(response.body);
      for await (const sseChunk of iterSSEChunks(iter)) {
          for (const line of lineDecoder.decode(sseChunk)) {
              const sse = sseDecoder.decode(line);
              if (sse)
                  yield sse;
          }
      }
      for (const line of lineDecoder.flush()) {
          const sse = sseDecoder.decode(line);
          if (sse)
              yield sse;
      }
  }
  /**
   * Given an async iterable iterator, iterates over it and yields full
   * SSE chunks, i.e. yields when a double new-line is encountered.
   */
  async function* iterSSEChunks(iterator) {
      let data = new Uint8Array();
      for await (const chunk of iterator) {
          if (chunk == null) {
              continue;
          }
          const binaryChunk = chunk instanceof ArrayBuffer ? new Uint8Array(chunk)
              : typeof chunk === 'string' ? new TextEncoder().encode(chunk)
                  : chunk;
          let newData = new Uint8Array(data.length + binaryChunk.length);
          newData.set(data);
          newData.set(binaryChunk, data.length);
          data = newData;
          let patternIndex;
          while ((patternIndex = findDoubleNewlineIndex(data)) !== -1) {
              yield data.slice(0, patternIndex);
              data = data.slice(patternIndex);
          }
      }
      if (data.length > 0) {
          yield data;
      }
  }
  function findDoubleNewlineIndex(buffer) {
      // This function searches the buffer for the end patterns (\r\r, \n\n, \r\n\r\n)
      // and returns the index right after the first occurrence of any pattern,
      // or -1 if none of the patterns are found.
      const newline = 0x0a; // \n
      const carriage = 0x0d; // \r
      for (let i = 0; i < buffer.length - 2; i++) {
          if (buffer[i] === newline && buffer[i + 1] === newline) {
              // \n\n
              return i + 2;
          }
          if (buffer[i] === carriage && buffer[i + 1] === carriage) {
              // \r\r
              return i + 2;
          }
          if (buffer[i] === carriage &&
              buffer[i + 1] === newline &&
              i + 3 < buffer.length &&
              buffer[i + 2] === carriage &&
              buffer[i + 3] === newline) {
              // \r\n\r\n
              return i + 4;
          }
      }
      return -1;
  }
  class SSEDecoder {
      constructor() {
          this.event = null;
          this.data = [];
          this.chunks = [];
      }
      decode(line) {
          if (line.endsWith('\r')) {
              line = line.substring(0, line.length - 1);
          }
          if (!line) {
              // empty line and we didn't previously encounter any messages
              if (!this.event && !this.data.length)
                  return null;
              const sse = {
                  event: this.event,
                  data: this.data.join('\n'),
                  raw: this.chunks,
              };
              this.event = null;
              this.data = [];
              this.chunks = [];
              return sse;
          }
          this.chunks.push(line);
          if (line.startsWith(':')) {
              return null;
          }
          let [fieldname, _, value] = partition(line, ':');
          if (value.startsWith(' ')) {
              value = value.substring(1);
          }
          if (fieldname === 'event') {
              this.event = value;
          }
          else if (fieldname === 'data') {
              this.data.push(value);
          }
          return null;
      }
  }
  function partition(str, delimiter) {
      const index = str.indexOf(delimiter);
      if (index !== -1) {
          return [str.substring(0, index), delimiter, str.substring(index + delimiter.length)];
      }
      return [str, '', ''];
  }

  const isResponseLike = (value) => value != null &&
      typeof value === 'object' &&
      typeof value.url === 'string' &&
      typeof value.blob === 'function';
  const isFileLike = (value) => value != null &&
      typeof value === 'object' &&
      typeof value.name === 'string' &&
      typeof value.lastModified === 'number' &&
      isBlobLike(value);
  /**
   * The BlobLike type omits arrayBuffer() because @types/node-fetch@^2.6.4 lacks it; but this check
   * adds the arrayBuffer() method type because it is available and used at runtime
   */
  const isBlobLike = (value) => value != null &&
      typeof value === 'object' &&
      typeof value.size === 'number' &&
      typeof value.type === 'string' &&
      typeof value.text === 'function' &&
      typeof value.slice === 'function' &&
      typeof value.arrayBuffer === 'function';
  /**
   * Helper for creating a {@link File} to pass to an SDK upload method from a variety of different data formats
   * @param value the raw content of the file.  Can be an {@link Uploadable}, {@link BlobLikePart}, or {@link AsyncIterable} of {@link BlobLikePart}s
   * @param {string=} name the name of the file. If omitted, toFile will try to determine a file name from bits if possible
   * @param {Object=} options additional properties
   * @param {string=} options.type the MIME type of the content
   * @param {number=} options.lastModified the last modified timestamp
   * @returns a {@link File} with the given properties
   */
  async function toFile(value, name, options) {
      // If it's a promise, resolve it.
      value = await value;
      // If we've been given a `File` we don't need to do anything
      if (isFileLike(value)) {
          return value;
      }
      if (isResponseLike(value)) {
          const blob = await value.blob();
          name || (name = new URL(value.url).pathname.split(/[\\/]/).pop() ?? 'unknown_file');
          // we need to convert the `Blob` into an array buffer because the `Blob` class
          // that `node-fetch` defines is incompatible with the web standard which results
          // in `new File` interpreting it as a string instead of binary data.
          const data = isBlobLike(blob) ? [(await blob.arrayBuffer())] : [blob];
          return new File$1(data, name, options);
      }
      const bits = await getBytes(value);
      name || (name = getName(value) ?? 'unknown_file');
      if (!options?.type) {
          const type = bits[0]?.type;
          if (typeof type === 'string') {
              options = { ...options, type };
          }
      }
      return new File$1(bits, name, options);
  }
  async function getBytes(value) {
      let parts = [];
      if (typeof value === 'string' ||
          ArrayBuffer.isView(value) || // includes Uint8Array, Buffer, etc.
          value instanceof ArrayBuffer) {
          parts.push(value);
      }
      else if (isBlobLike(value)) {
          parts.push(await value.arrayBuffer());
      }
      else if (isAsyncIterableIterator(value) // includes Readable, ReadableStream, etc.
      ) {
          for await (const chunk of value) {
              parts.push(chunk); // TODO, consider validating?
          }
      }
      else {
          throw new Error(`Unexpected data type: ${typeof value}; constructor: ${value?.constructor
            ?.name}; props: ${propsForError(value)}`);
      }
      return parts;
  }
  function propsForError(value) {
      const props = Object.getOwnPropertyNames(value);
      return `[${props.map((p) => `"${p}"`).join(', ')}]`;
  }
  function getName(value) {
      return (getStringFromMaybeBuffer(value.name) ||
          getStringFromMaybeBuffer(value.filename) ||
          // For fs.ReadStream
          getStringFromMaybeBuffer(value.path)?.split(/[\\/]/).pop());
  }
  const getStringFromMaybeBuffer = (x) => {
      if (typeof x === 'string')
          return x;
      if (typeof bufferExports.Buffer !== 'undefined' && x instanceof bufferExports.Buffer)
          return String(x);
      return undefined;
  };
  const isAsyncIterableIterator = (value) => value != null && typeof value === 'object' && typeof value[Symbol.asyncIterator] === 'function';
  const isMultipartBody = (body) => body && typeof body === 'object' && body.body && body[Symbol.toStringTag] === 'MultipartBody';

  var __classPrivateFieldSet$2 = (undefined && undefined.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
      if (kind === "m") throw new TypeError("Private method is not writable");
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
      return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
  };
  var __classPrivateFieldGet$2 = (undefined && undefined.__classPrivateFieldGet) || function (receiver, state, kind, f) {
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
      return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
  };
  var _AbstractPage_client;
  async function defaultParseResponse(props) {
      const { response } = props;
      if (props.options.stream) {
          debug('response', response.status, response.url, response.headers, response.body);
          // Note: there is an invariant here that isn't represented in the type system
          // that if you set `stream: true` the response type must also be `Stream<T>`
          if (props.options.__streamClass) {
              return props.options.__streamClass.fromSSEResponse(response, props.controller);
          }
          return Stream.fromSSEResponse(response, props.controller);
      }
      // fetch refuses to read the body when the status code is 204.
      if (response.status === 204) {
          return null;
      }
      if (props.options.__binaryResponse) {
          return response;
      }
      const contentType = response.headers.get('content-type');
      const isJSON = contentType?.includes('application/json') || contentType?.includes('application/vnd.api+json');
      if (isJSON) {
          const json = await response.json();
          debug('response', response.status, response.url, response.headers, json);
          return _addRequestID(json, response);
      }
      const text = await response.text();
      debug('response', response.status, response.url, response.headers, text);
      // TODO handle blob, arraybuffer, other content types, etc.
      return text;
  }
  function _addRequestID(value, response) {
      if (!value || typeof value !== 'object' || Array.isArray(value)) {
          return value;
      }
      return Object.defineProperty(value, '_request_id', {
          value: response.headers.get('request-id'),
          enumerable: false,
      });
  }
  /**
   * A subclass of `Promise` providing additional helper methods
   * for interacting with the SDK.
   */
  class APIPromise extends Promise {
      constructor(responsePromise, parseResponse = defaultParseResponse) {
          super((resolve) => {
              // this is maybe a bit weird but this has to be a no-op to not implicitly
              // parse the response body; instead .then, .catch, .finally are overridden
              // to parse the response
              resolve(null);
          });
          this.responsePromise = responsePromise;
          this.parseResponse = parseResponse;
      }
      _thenUnwrap(transform) {
          return new APIPromise(this.responsePromise, async (props) => _addRequestID(transform(await this.parseResponse(props), props), props.response));
      }
      /**
       * Gets the raw `Response` instance instead of parsing the response
       * data.
       *
       * If you want to parse the response body but still get the `Response`
       * instance, you can use {@link withResponse()}.
       *
       * 👋 Getting the wrong TypeScript type for `Response`?
       * Try setting `"moduleResolution": "NodeNext"` if you can,
       * or add one of these imports before your first `import … from '@anthropic-ai/sdk'`:
       * - `import '@anthropic-ai/sdk/shims/node'` (if you're running on Node)
       * - `import '@anthropic-ai/sdk/shims/web'` (otherwise)
       */
      asResponse() {
          return this.responsePromise.then((p) => p.response);
      }
      /**
       * Gets the parsed response data, the raw `Response` instance and the ID of the request,
       * returned vie the `request-id` header which is useful for debugging requests and resporting
       * issues to Anthropic.
       *
       * If you just want to get the raw `Response` instance without parsing it,
       * you can use {@link asResponse()}.
       *
       * 👋 Getting the wrong TypeScript type for `Response`?
       * Try setting `"moduleResolution": "NodeNext"` if you can,
       * or add one of these imports before your first `import … from '@anthropic-ai/sdk'`:
       * - `import '@anthropic-ai/sdk/shims/node'` (if you're running on Node)
       * - `import '@anthropic-ai/sdk/shims/web'` (otherwise)
       */
      async withResponse() {
          const [data, response] = await Promise.all([this.parse(), this.asResponse()]);
          return { data, response, request_id: response.headers.get('request-id') };
      }
      parse() {
          if (!this.parsedPromise) {
              this.parsedPromise = this.responsePromise.then(this.parseResponse);
          }
          return this.parsedPromise;
      }
      then(onfulfilled, onrejected) {
          return this.parse().then(onfulfilled, onrejected);
      }
      catch(onrejected) {
          return this.parse().catch(onrejected);
      }
      finally(onfinally) {
          return this.parse().finally(onfinally);
      }
  }
  class APIClient {
      constructor({ baseURL, maxRetries = 2, timeout = 600000, // 10 minutes
      httpAgent, fetch: overriddenFetch, }) {
          this.baseURL = baseURL;
          this.maxRetries = validatePositiveInteger('maxRetries', maxRetries);
          this.timeout = validatePositiveInteger('timeout', timeout);
          this.httpAgent = httpAgent;
          this.fetch = overriddenFetch ?? fetch$1;
      }
      authHeaders(opts) {
          return {};
      }
      /**
       * Override this to add your own default headers, for example:
       *
       *  {
       *    ...super.defaultHeaders(),
       *    Authorization: 'Bearer 123',
       *  }
       */
      defaultHeaders(opts) {
          return {
              Accept: 'application/json',
              'Content-Type': 'application/json',
              'User-Agent': this.getUserAgent(),
              ...getPlatformHeaders(),
              ...this.authHeaders(opts),
          };
      }
      /**
       * Override this to add your own headers validation:
       */
      validateHeaders(headers, customHeaders) { }
      defaultIdempotencyKey() {
          return `stainless-node-retry-${uuid4()}`;
      }
      get(path, opts) {
          return this.methodRequest('get', path, opts);
      }
      post(path, opts) {
          return this.methodRequest('post', path, opts);
      }
      patch(path, opts) {
          return this.methodRequest('patch', path, opts);
      }
      put(path, opts) {
          return this.methodRequest('put', path, opts);
      }
      delete(path, opts) {
          return this.methodRequest('delete', path, opts);
      }
      methodRequest(method, path, opts) {
          return this.request(Promise.resolve(opts).then(async (opts) => {
              const body = opts && isBlobLike(opts?.body) ? new DataView(await opts.body.arrayBuffer())
                  : opts?.body instanceof DataView ? opts.body
                      : opts?.body instanceof ArrayBuffer ? new DataView(opts.body)
                          : opts && ArrayBuffer.isView(opts?.body) ? new DataView(opts.body.buffer)
                              : opts?.body;
              return { method, path, ...opts, body };
          }));
      }
      getAPIList(path, Page, opts) {
          return this.requestAPIList(Page, { method: 'get', path, ...opts });
      }
      calculateContentLength(body) {
          if (typeof body === 'string') {
              if (typeof bufferExports.Buffer !== 'undefined') {
                  return bufferExports.Buffer.byteLength(body, 'utf8').toString();
              }
              if (typeof TextEncoder !== 'undefined') {
                  const encoder = new TextEncoder();
                  const encoded = encoder.encode(body);
                  return encoded.length.toString();
              }
          }
          else if (ArrayBuffer.isView(body)) {
              return body.byteLength.toString();
          }
          return null;
      }
      buildRequest(options, { retryCount = 0 } = {}) {
          const { method, path, query, headers: headers = {} } = options;
          const body = ArrayBuffer.isView(options.body) || (options.__binaryRequest && typeof options.body === 'string') ?
              options.body
              : isMultipartBody(options.body) ? options.body.body
                  : options.body ? JSON.stringify(options.body, null, 2)
                      : null;
          const contentLength = this.calculateContentLength(body);
          const url = this.buildURL(path, query);
          if ('timeout' in options)
              validatePositiveInteger('timeout', options.timeout);
          const timeout = options.timeout ?? this.timeout;
          const httpAgent = options.httpAgent ?? this.httpAgent ?? getDefaultAgent(url);
          const minAgentTimeout = timeout + 1000;
          if (typeof httpAgent?.options?.timeout === 'number' &&
              minAgentTimeout > (httpAgent.options.timeout ?? 0)) {
              // Allow any given request to bump our agent active socket timeout.
              // This may seem strange, but leaking active sockets should be rare and not particularly problematic,
              // and without mutating agent we would need to create more of them.
              // This tradeoff optimizes for performance.
              httpAgent.options.timeout = minAgentTimeout;
          }
          if (this.idempotencyHeader && method !== 'get') {
              if (!options.idempotencyKey)
                  options.idempotencyKey = this.defaultIdempotencyKey();
              headers[this.idempotencyHeader] = options.idempotencyKey;
          }
          const reqHeaders = this.buildHeaders({ options, headers, contentLength, retryCount });
          const req = {
              method,
              ...(body && { body: body }),
              headers: reqHeaders,
              ...(httpAgent && { agent: httpAgent }),
              // @ts-ignore node-fetch uses a custom AbortSignal type that is
              // not compatible with standard web types
              signal: options.signal ?? null,
          };
          return { req, url, timeout };
      }
      buildHeaders({ options, headers, contentLength, retryCount, }) {
          const reqHeaders = {};
          if (contentLength) {
              reqHeaders['content-length'] = contentLength;
          }
          const defaultHeaders = this.defaultHeaders(options);
          applyHeadersMut(reqHeaders, defaultHeaders);
          applyHeadersMut(reqHeaders, headers);
          // let builtin fetch set the Content-Type for multipart bodies
          if (isMultipartBody(options.body) && kind !== 'node') {
              delete reqHeaders['content-type'];
          }
          // Don't set the retry count header if it was already set or removed through default headers or by the
          // caller. We check `defaultHeaders` and `headers`, which can contain nulls, instead of `reqHeaders` to
          // account for the removal case.
          if (getHeader(defaultHeaders, 'x-stainless-retry-count') === undefined &&
              getHeader(headers, 'x-stainless-retry-count') === undefined) {
              reqHeaders['x-stainless-retry-count'] = String(retryCount);
          }
          this.validateHeaders(reqHeaders, headers);
          return reqHeaders;
      }
      /**
       * Used as a callback for mutating the given `FinalRequestOptions` object.
       */
      async prepareOptions(options) { }
      /**
       * Used as a callback for mutating the given `RequestInit` object.
       *
       * This is useful for cases where you want to add certain headers based off of
       * the request properties, e.g. `method` or `url`.
       */
      async prepareRequest(request, { url, options }) { }
      parseHeaders(headers) {
          return (!headers ? {}
              : Symbol.iterator in headers ?
                  Object.fromEntries(Array.from(headers).map((header) => [...header]))
                  : { ...headers });
      }
      makeStatusError(status, error, message, headers) {
          return APIError.generate(status, error, message, headers);
      }
      request(options, remainingRetries = null) {
          return new APIPromise(this.makeRequest(options, remainingRetries));
      }
      async makeRequest(optionsInput, retriesRemaining) {
          const options = await optionsInput;
          const maxRetries = options.maxRetries ?? this.maxRetries;
          if (retriesRemaining == null) {
              retriesRemaining = maxRetries;
          }
          await this.prepareOptions(options);
          const { req, url, timeout } = this.buildRequest(options, { retryCount: maxRetries - retriesRemaining });
          await this.prepareRequest(req, { url, options });
          debug('request', url, options, req.headers);
          if (options.signal?.aborted) {
              throw new APIUserAbortError();
          }
          const controller = new AbortController();
          const response = await this.fetchWithTimeout(url, req, timeout, controller).catch(castToError);
          if (response instanceof Error) {
              if (options.signal?.aborted) {
                  throw new APIUserAbortError();
              }
              if (retriesRemaining) {
                  return this.retryRequest(options, retriesRemaining);
              }
              if (response.name === 'AbortError') {
                  throw new APIConnectionTimeoutError();
              }
              throw new APIConnectionError({ cause: response });
          }
          const responseHeaders = createResponseHeaders(response.headers);
          if (!response.ok) {
              if (retriesRemaining && this.shouldRetry(response)) {
                  const retryMessage = `retrying, ${retriesRemaining} attempts remaining`;
                  debug(`response (error; ${retryMessage})`, response.status, url, responseHeaders);
                  return this.retryRequest(options, retriesRemaining, responseHeaders);
              }
              const errText = await response.text().catch((e) => castToError(e).message);
              const errJSON = safeJSON(errText);
              const errMessage = errJSON ? undefined : errText;
              const retryMessage = retriesRemaining ? `(error; no more retries left)` : `(error; not retryable)`;
              debug(`response (error; ${retryMessage})`, response.status, url, responseHeaders, errMessage);
              const err = this.makeStatusError(response.status, errJSON, errMessage, responseHeaders);
              throw err;
          }
          return { response, options, controller };
      }
      requestAPIList(Page, options) {
          const request = this.makeRequest(options, null);
          return new PagePromise(this, request, Page);
      }
      buildURL(path, query) {
          const url = isAbsoluteURL(path) ?
              new URL(path)
              : new URL(this.baseURL + (this.baseURL.endsWith('/') && path.startsWith('/') ? path.slice(1) : path));
          const defaultQuery = this.defaultQuery();
          if (!isEmptyObj(defaultQuery)) {
              query = { ...defaultQuery, ...query };
          }
          if (typeof query === 'object' && query && !Array.isArray(query)) {
              url.search = this.stringifyQuery(query);
          }
          return url.toString();
      }
      stringifyQuery(query) {
          return Object.entries(query)
              .filter(([_, value]) => typeof value !== 'undefined')
              .map(([key, value]) => {
              if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
                  return `${encodeURIComponent(key)}=${encodeURIComponent(value)}`;
              }
              if (value === null) {
                  return `${encodeURIComponent(key)}=`;
              }
              throw new AnthropicError(`Cannot stringify type ${typeof value}; Expected string, number, boolean, or null. If you need to pass nested query parameters, you can manually encode them, e.g. { query: { 'foo[key1]': value1, 'foo[key2]': value2 } }, and please open a GitHub issue requesting better support for your use case.`);
          })
              .join('&');
      }
      async fetchWithTimeout(url, init, ms, controller) {
          const { signal, ...options } = init || {};
          if (signal)
              signal.addEventListener('abort', () => controller.abort());
          const timeout = setTimeout(() => controller.abort(), ms);
          const fetchOptions = {
              signal: controller.signal,
              ...options,
          };
          if (fetchOptions.method) {
              // Custom methods like 'patch' need to be uppercased
              // See https://github.com/nodejs/undici/issues/2294
              fetchOptions.method = fetchOptions.method.toUpperCase();
          }
          return (
          // use undefined this binding; fetch errors if bound to something else in browser/cloudflare
          this.fetch.call(undefined, url, fetchOptions).finally(() => {
              clearTimeout(timeout);
          }));
      }
      shouldRetry(response) {
          // Note this is not a standard header.
          const shouldRetryHeader = response.headers.get('x-should-retry');
          // If the server explicitly says whether or not to retry, obey.
          if (shouldRetryHeader === 'true')
              return true;
          if (shouldRetryHeader === 'false')
              return false;
          // Retry on request timeouts.
          if (response.status === 408)
              return true;
          // Retry on lock timeouts.
          if (response.status === 409)
              return true;
          // Retry on rate limits.
          if (response.status === 429)
              return true;
          // Retry internal errors.
          if (response.status >= 500)
              return true;
          return false;
      }
      async retryRequest(options, retriesRemaining, responseHeaders) {
          let timeoutMillis;
          // Note the `retry-after-ms` header may not be standard, but is a good idea and we'd like proactive support for it.
          const retryAfterMillisHeader = responseHeaders?.['retry-after-ms'];
          if (retryAfterMillisHeader) {
              const timeoutMs = parseFloat(retryAfterMillisHeader);
              if (!Number.isNaN(timeoutMs)) {
                  timeoutMillis = timeoutMs;
              }
          }
          // About the Retry-After header: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After
          const retryAfterHeader = responseHeaders?.['retry-after'];
          if (retryAfterHeader && !timeoutMillis) {
              const timeoutSeconds = parseFloat(retryAfterHeader);
              if (!Number.isNaN(timeoutSeconds)) {
                  timeoutMillis = timeoutSeconds * 1000;
              }
              else {
                  timeoutMillis = Date.parse(retryAfterHeader) - Date.now();
              }
          }
          // If the API asks us to wait a certain amount of time (and it's a reasonable amount),
          // just do what it says, but otherwise calculate a default
          if (!(timeoutMillis && 0 <= timeoutMillis && timeoutMillis < 60 * 1000)) {
              const maxRetries = options.maxRetries ?? this.maxRetries;
              timeoutMillis = this.calculateDefaultRetryTimeoutMillis(retriesRemaining, maxRetries);
          }
          await sleep(timeoutMillis);
          return this.makeRequest(options, retriesRemaining - 1);
      }
      calculateDefaultRetryTimeoutMillis(retriesRemaining, maxRetries) {
          const initialRetryDelay = 0.5;
          const maxRetryDelay = 8.0;
          const numRetries = maxRetries - retriesRemaining;
          // Apply exponential backoff, but not more than the max.
          const sleepSeconds = Math.min(initialRetryDelay * Math.pow(2, numRetries), maxRetryDelay);
          // Apply some jitter, take up to at most 25 percent of the retry time.
          const jitter = 1 - Math.random() * 0.25;
          return sleepSeconds * jitter * 1000;
      }
      getUserAgent() {
          return `${this.constructor.name}/JS ${VERSION}`;
      }
  }
  class AbstractPage {
      constructor(client, response, body, options) {
          _AbstractPage_client.set(this, void 0);
          __classPrivateFieldSet$2(this, _AbstractPage_client, client, "f");
          this.options = options;
          this.response = response;
          this.body = body;
      }
      hasNextPage() {
          const items = this.getPaginatedItems();
          if (!items.length)
              return false;
          return this.nextPageInfo() != null;
      }
      async getNextPage() {
          const nextInfo = this.nextPageInfo();
          if (!nextInfo) {
              throw new AnthropicError('No next page expected; please check `.hasNextPage()` before calling `.getNextPage()`.');
          }
          const nextOptions = { ...this.options };
          if ('params' in nextInfo && typeof nextOptions.query === 'object') {
              nextOptions.query = { ...nextOptions.query, ...nextInfo.params };
          }
          else if ('url' in nextInfo) {
              const params = [...Object.entries(nextOptions.query || {}), ...nextInfo.url.searchParams.entries()];
              for (const [key, value] of params) {
                  nextInfo.url.searchParams.set(key, value);
              }
              nextOptions.query = undefined;
              nextOptions.path = nextInfo.url.toString();
          }
          return await __classPrivateFieldGet$2(this, _AbstractPage_client, "f").requestAPIList(this.constructor, nextOptions);
      }
      async *iterPages() {
          // eslint-disable-next-line @typescript-eslint/no-this-alias
          let page = this;
          yield page;
          while (page.hasNextPage()) {
              page = await page.getNextPage();
              yield page;
          }
      }
      async *[(_AbstractPage_client = new WeakMap(), Symbol.asyncIterator)]() {
          for await (const page of this.iterPages()) {
              for (const item of page.getPaginatedItems()) {
                  yield item;
              }
          }
      }
  }
  /**
   * This subclass of Promise will resolve to an instantiated Page once the request completes.
   *
   * It also implements AsyncIterable to allow auto-paginating iteration on an unawaited list call, eg:
   *
   *    for await (const item of client.items.list()) {
   *      console.log(item)
   *    }
   */
  class PagePromise extends APIPromise {
      constructor(client, request, Page) {
          super(request, async (props) => new Page(client, props.response, await defaultParseResponse(props), props.options));
      }
      /**
       * Allow auto-paginating iteration on an unawaited list call, eg:
       *
       *    for await (const item of client.items.list()) {
       *      console.log(item)
       *    }
       */
      async *[Symbol.asyncIterator]() {
          const page = await this;
          for await (const item of page) {
              yield item;
          }
      }
  }
  const createResponseHeaders = (headers) => {
      return new Proxy(Object.fromEntries(
      // @ts-ignore
      headers.entries()), {
          get(target, name) {
              const key = name.toString();
              return target[key.toLowerCase()] || target[key];
          },
      });
  };
  // This is required so that we can determine if a given object matches the RequestOptions
  // type at runtime. While this requires duplication, it is enforced by the TypeScript
  // compiler such that any missing / extraneous keys will cause an error.
  const requestOptionsKeys = {
      method: true,
      path: true,
      query: true,
      body: true,
      headers: true,
      maxRetries: true,
      stream: true,
      timeout: true,
      httpAgent: true,
      signal: true,
      idempotencyKey: true,
      __binaryRequest: true,
      __binaryResponse: true,
      __streamClass: true,
  };
  const isRequestOptions = (obj) => {
      return (typeof obj === 'object' &&
          obj !== null &&
          !isEmptyObj(obj) &&
          Object.keys(obj).every((k) => hasOwn(requestOptionsKeys, k)));
  };
  const getPlatformProperties = () => {
      if (typeof Deno !== 'undefined' && Deno.build != null) {
          return {
              'X-Stainless-Lang': 'js',
              'X-Stainless-Package-Version': VERSION,
              'X-Stainless-OS': normalizePlatform(Deno.build.os),
              'X-Stainless-Arch': normalizeArch(Deno.build.arch),
              'X-Stainless-Runtime': 'deno',
              'X-Stainless-Runtime-Version': typeof Deno.version === 'string' ? Deno.version : Deno.version?.deno ?? 'unknown',
          };
      }
      if (typeof EdgeRuntime !== 'undefined') {
          return {
              'X-Stainless-Lang': 'js',
              'X-Stainless-Package-Version': VERSION,
              'X-Stainless-OS': 'Unknown',
              'X-Stainless-Arch': `other:${EdgeRuntime}`,
              'X-Stainless-Runtime': 'edge',
              'X-Stainless-Runtime-Version': process.version,
          };
      }
      // Check if Node.js
      if (Object.prototype.toString.call(typeof process !== 'undefined' ? process : 0) === '[object process]') {
          return {
              'X-Stainless-Lang': 'js',
              'X-Stainless-Package-Version': VERSION,
              'X-Stainless-OS': normalizePlatform(process.platform),
              'X-Stainless-Arch': normalizeArch(process.arch),
              'X-Stainless-Runtime': 'node',
              'X-Stainless-Runtime-Version': process.version,
          };
      }
      const browserInfo = getBrowserInfo();
      if (browserInfo) {
          return {
              'X-Stainless-Lang': 'js',
              'X-Stainless-Package-Version': VERSION,
              'X-Stainless-OS': 'Unknown',
              'X-Stainless-Arch': 'unknown',
              'X-Stainless-Runtime': `browser:${browserInfo.browser}`,
              'X-Stainless-Runtime-Version': browserInfo.version,
          };
      }
      // TODO add support for Cloudflare workers, etc.
      return {
          'X-Stainless-Lang': 'js',
          'X-Stainless-Package-Version': VERSION,
          'X-Stainless-OS': 'Unknown',
          'X-Stainless-Arch': 'unknown',
          'X-Stainless-Runtime': 'unknown',
          'X-Stainless-Runtime-Version': 'unknown',
      };
  };
  // Note: modified from https://github.com/JS-DevTools/host-environment/blob/b1ab79ecde37db5d6e163c050e54fe7d287d7c92/src/isomorphic.browser.ts
  function getBrowserInfo() {
      if (typeof navigator === 'undefined' || !navigator) {
          return null;
      }
      // NOTE: The order matters here!
      const browserPatterns = [
          { key: 'edge', pattern: /Edge(?:\W+(\d+)\.(\d+)(?:\.(\d+))?)?/ },
          { key: 'ie', pattern: /MSIE(?:\W+(\d+)\.(\d+)(?:\.(\d+))?)?/ },
          { key: 'ie', pattern: /Trident(?:.*rv\:(\d+)\.(\d+)(?:\.(\d+))?)?/ },
          { key: 'chrome', pattern: /Chrome(?:\W+(\d+)\.(\d+)(?:\.(\d+))?)?/ },
          { key: 'firefox', pattern: /Firefox(?:\W+(\d+)\.(\d+)(?:\.(\d+))?)?/ },
          { key: 'safari', pattern: /(?:Version\W+(\d+)\.(\d+)(?:\.(\d+))?)?(?:\W+Mobile\S*)?\W+Safari/ },
      ];
      // Find the FIRST matching browser
      for (const { key, pattern } of browserPatterns) {
          const match = pattern.exec(navigator.userAgent);
          if (match) {
              const major = match[1] || 0;
              const minor = match[2] || 0;
              const patch = match[3] || 0;
              return { browser: key, version: `${major}.${minor}.${patch}` };
          }
      }
      return null;
  }
  const normalizeArch = (arch) => {
      // Node docs:
      // - https://nodejs.org/api/process.html#processarch
      // Deno docs:
      // - https://doc.deno.land/deno/stable/~/Deno.build
      if (arch === 'x32')
          return 'x32';
      if (arch === 'x86_64' || arch === 'x64')
          return 'x64';
      if (arch === 'arm')
          return 'arm';
      if (arch === 'aarch64' || arch === 'arm64')
          return 'arm64';
      if (arch)
          return `other:${arch}`;
      return 'unknown';
  };
  const normalizePlatform = (platform) => {
      // Node platforms:
      // - https://nodejs.org/api/process.html#processplatform
      // Deno platforms:
      // - https://doc.deno.land/deno/stable/~/Deno.build
      // - https://github.com/denoland/deno/issues/14799
      platform = platform.toLowerCase();
      // NOTE: this iOS check is untested and may not work
      // Node does not work natively on IOS, there is a fork at
      // https://github.com/nodejs-mobile/nodejs-mobile
      // however it is unknown at the time of writing how to detect if it is running
      if (platform.includes('ios'))
          return 'iOS';
      if (platform === 'android')
          return 'Android';
      if (platform === 'darwin')
          return 'MacOS';
      if (platform === 'win32')
          return 'Windows';
      if (platform === 'freebsd')
          return 'FreeBSD';
      if (platform === 'openbsd')
          return 'OpenBSD';
      if (platform === 'linux')
          return 'Linux';
      if (platform)
          return `Other:${platform}`;
      return 'Unknown';
  };
  let _platformHeaders;
  const getPlatformHeaders = () => {
      return (_platformHeaders ?? (_platformHeaders = getPlatformProperties()));
  };
  const safeJSON = (text) => {
      try {
          return JSON.parse(text);
      }
      catch (err) {
          return undefined;
      }
  };
  // https://url.spec.whatwg.org/#url-scheme-string
  const startsWithSchemeRegexp = /^[a-z][a-z0-9+.-]*:/i;
  const isAbsoluteURL = (url) => {
      return startsWithSchemeRegexp.test(url);
  };
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const validatePositiveInteger = (name, n) => {
      if (typeof n !== 'number' || !Number.isInteger(n)) {
          throw new AnthropicError(`${name} must be an integer`);
      }
      if (n < 0) {
          throw new AnthropicError(`${name} must be a positive integer`);
      }
      return n;
  };
  const castToError = (err) => {
      if (err instanceof Error)
          return err;
      if (typeof err === 'object' && err !== null) {
          try {
              return new Error(JSON.stringify(err));
          }
          catch { }
      }
      return new Error(String(err));
  };
  /**
   * Read an environment variable.
   *
   * Trims beginning and trailing whitespace.
   *
   * Will return undefined if the environment variable doesn't exist or cannot be accessed.
   */
  const readEnv = (env) => {
      if (typeof process !== 'undefined') {
          return process.env?.[env]?.trim() ?? undefined;
      }
      if (typeof Deno !== 'undefined') {
          return Deno.env?.get?.(env)?.trim();
      }
      return undefined;
  };
  // https://stackoverflow.com/a/34491287
  function isEmptyObj(obj) {
      if (!obj)
          return true;
      for (const _k in obj)
          return false;
      return true;
  }
  // https://eslint.org/docs/latest/rules/no-prototype-builtins
  function hasOwn(obj, key) {
      return Object.prototype.hasOwnProperty.call(obj, key);
  }
  /**
   * Copies headers from "newHeaders" onto "targetHeaders",
   * using lower-case for all properties,
   * ignoring any keys with undefined values,
   * and deleting any keys with null values.
   */
  function applyHeadersMut(targetHeaders, newHeaders) {
      for (const k in newHeaders) {
          if (!hasOwn(newHeaders, k))
              continue;
          const lowerKey = k.toLowerCase();
          if (!lowerKey)
              continue;
          const val = newHeaders[k];
          if (val === null) {
              delete targetHeaders[lowerKey];
          }
          else if (val !== undefined) {
              targetHeaders[lowerKey] = val;
          }
      }
  }
  function debug(action, ...args) {
      if (typeof process !== 'undefined' && process?.env?.['DEBUG'] === 'true') {
          console.log(`Anthropic:DEBUG:${action}`, ...args);
      }
  }
  /**
   * https://stackoverflow.com/a/2117523
   */
  const uuid4 = () => {
      return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
          const r = (Math.random() * 16) | 0;
          const v = c === 'x' ? r : (r & 0x3) | 0x8;
          return v.toString(16);
      });
  };
  const isRunningInBrowser = () => {
      return (
      // @ts-ignore
      typeof window !== 'undefined' &&
          // @ts-ignore
          typeof window.document !== 'undefined' &&
          // @ts-ignore
          typeof navigator !== 'undefined');
  };
  const isHeadersProtocol = (headers) => {
      return typeof headers?.get === 'function';
  };
  const getHeader = (headers, header) => {
      const lowerCasedHeader = header.toLowerCase();
      if (isHeadersProtocol(headers)) {
          // to deal with the case where the header looks like Stainless-Event-Id
          const intercapsHeader = header[0]?.toUpperCase() +
              header.substring(1).replace(/([^\w])(\w)/g, (_m, g1, g2) => g1 + g2.toUpperCase());
          for (const key of [header, lowerCasedHeader, header.toUpperCase(), intercapsHeader]) {
              const value = headers.get(key);
              if (value) {
                  return value;
              }
          }
      }
      for (const [key, value] of Object.entries(headers)) {
          if (key.toLowerCase() === lowerCasedHeader) {
              if (Array.isArray(value)) {
                  if (value.length <= 1)
                      return value[0];
                  console.warn(`Received ${value.length} entries for the ${header} header, using the first entry.`);
                  return value[0];
              }
              return value;
          }
      }
      return undefined;
  };

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Page extends AbstractPage {
      constructor(client, response, body, options) {
          super(client, response, body, options);
          this.data = body.data || [];
          this.has_more = body.has_more || false;
          this.first_id = body.first_id || null;
          this.last_id = body.last_id || null;
      }
      getPaginatedItems() {
          return this.data ?? [];
      }
      // @deprecated Please use `nextPageInfo()` instead
      nextPageParams() {
          const info = this.nextPageInfo();
          if (!info)
              return null;
          if ('params' in info)
              return info.params;
          const params = Object.fromEntries(info.url.searchParams);
          if (!Object.keys(params).length)
              return null;
          return params;
      }
      nextPageInfo() {
          if (this.options.query?.['before_id']) {
              // in reverse
              const firstId = this.first_id;
              if (!firstId) {
                  return null;
              }
              return {
                  params: {
                      before_id: firstId,
                  },
              };
          }
          const cursor = this.last_id;
          if (!cursor) {
              return null;
          }
          return {
              params: {
                  after_id: cursor,
              },
          };
      }
  }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class APIResource {
      constructor(client) {
          this._client = client;
      }
  }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  let Models$1 = class Models extends APIResource {
      /**
       * Get a specific model.
       *
       * The Models API response can be used to determine information about a specific
       * model or resolve a model alias to a model ID.
       */
      retrieve(modelId, options) {
          return this._client.get(`/v1/models/${modelId}?beta=true`, options);
      }
      list(query = {}, options) {
          if (isRequestOptions(query)) {
              return this.list({}, query);
          }
          return this._client.getAPIList('/v1/models?beta=true', BetaModelInfosPage, { query, ...options });
      }
  };
  class BetaModelInfosPage extends Page {
  }
  Models$1.BetaModelInfosPage = BetaModelInfosPage;

  class JSONLDecoder {
      constructor(iterator, controller) {
          this.iterator = iterator;
          this.controller = controller;
      }
      async *decoder() {
          const lineDecoder = new LineDecoder();
          for await (const chunk of this.iterator) {
              for (const line of lineDecoder.decode(chunk)) {
                  yield JSON.parse(line);
              }
          }
          for (const line of lineDecoder.flush()) {
              yield JSON.parse(line);
          }
      }
      [Symbol.asyncIterator]() {
          return this.decoder();
      }
      static fromResponse(response, controller) {
          if (!response.body) {
              controller.abort();
              throw new AnthropicError(`Attempted to iterate over a response with no body`);
          }
          return new JSONLDecoder(ReadableStreamToAsyncIterable(response.body), controller);
      }
  }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  let Batches$1 = class Batches extends APIResource {
      /**
       * Send a batch of Message creation requests.
       *
       * The Message Batches API can be used to process multiple Messages API requests at
       * once. Once a Message Batch is created, it begins processing immediately. Batches
       * can take up to 24 hours to complete.
       */
      create(params, options) {
          const { betas, ...body } = params;
          return this._client.post('/v1/messages/batches?beta=true', {
              body,
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  ...options?.headers,
              },
          });
      }
      retrieve(messageBatchId, params = {}, options) {
          if (isRequestOptions(params)) {
              return this.retrieve(messageBatchId, {}, params);
          }
          const { betas } = params;
          return this._client.get(`/v1/messages/batches/${messageBatchId}?beta=true`, {
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  ...options?.headers,
              },
          });
      }
      list(params = {}, options) {
          if (isRequestOptions(params)) {
              return this.list({}, params);
          }
          const { betas, ...query } = params;
          return this._client.getAPIList('/v1/messages/batches?beta=true', BetaMessageBatchesPage, {
              query,
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  ...options?.headers,
              },
          });
      }
      delete(messageBatchId, params = {}, options) {
          if (isRequestOptions(params)) {
              return this.delete(messageBatchId, {}, params);
          }
          const { betas } = params;
          return this._client.delete(`/v1/messages/batches/${messageBatchId}?beta=true`, {
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  ...options?.headers,
              },
          });
      }
      cancel(messageBatchId, params = {}, options) {
          if (isRequestOptions(params)) {
              return this.cancel(messageBatchId, {}, params);
          }
          const { betas } = params;
          return this._client.post(`/v1/messages/batches/${messageBatchId}/cancel?beta=true`, {
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  ...options?.headers,
              },
          });
      }
      async results(messageBatchId, params = {}, options) {
          if (isRequestOptions(params)) {
              return this.results(messageBatchId, {}, params);
          }
          const batch = await this.retrieve(messageBatchId);
          if (!batch.results_url) {
              throw new AnthropicError(`No batch \`results_url\`; Has it finished processing? ${batch.processing_status} - ${batch.id}`);
          }
          const { betas } = params;
          return this._client
              .get(batch.results_url, {
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'message-batches-2024-09-24'].toString(),
                  Accept: 'application/binary',
                  ...options?.headers,
              },
              __binaryResponse: true,
          })
              ._thenUnwrap((_, props) => JSONLDecoder.fromResponse(props.response, props.controller));
      }
  };
  class BetaMessageBatchesPage extends Page {
  }
  Batches$1.BetaMessageBatchesPage = BetaMessageBatchesPage;

  const tokenize = (input) => {
      let current = 0;
      let tokens = [];
      while (current < input.length) {
          let char = input[current];
          if (char === '\\') {
              current++;
              continue;
          }
          if (char === '{') {
              tokens.push({
                  type: 'brace',
                  value: '{',
              });
              current++;
              continue;
          }
          if (char === '}') {
              tokens.push({
                  type: 'brace',
                  value: '}',
              });
              current++;
              continue;
          }
          if (char === '[') {
              tokens.push({
                  type: 'paren',
                  value: '[',
              });
              current++;
              continue;
          }
          if (char === ']') {
              tokens.push({
                  type: 'paren',
                  value: ']',
              });
              current++;
              continue;
          }
          if (char === ':') {
              tokens.push({
                  type: 'separator',
                  value: ':',
              });
              current++;
              continue;
          }
          if (char === ',') {
              tokens.push({
                  type: 'delimiter',
                  value: ',',
              });
              current++;
              continue;
          }
          if (char === '"') {
              let value = '';
              let danglingQuote = false;
              char = input[++current];
              while (char !== '"') {
                  if (current === input.length) {
                      danglingQuote = true;
                      break;
                  }
                  if (char === '\\') {
                      current++;
                      if (current === input.length) {
                          danglingQuote = true;
                          break;
                      }
                      value += char + input[current];
                      char = input[++current];
                  }
                  else {
                      value += char;
                      char = input[++current];
                  }
              }
              char = input[++current];
              if (!danglingQuote) {
                  tokens.push({
                      type: 'string',
                      value,
                  });
              }
              continue;
          }
          let WHITESPACE = /\s/;
          if (char && WHITESPACE.test(char)) {
              current++;
              continue;
          }
          let NUMBERS = /[0-9]/;
          if ((char && NUMBERS.test(char)) || char === '-' || char === '.') {
              let value = '';
              if (char === '-') {
                  value += char;
                  char = input[++current];
              }
              while ((char && NUMBERS.test(char)) || char === '.') {
                  value += char;
                  char = input[++current];
              }
              tokens.push({
                  type: 'number',
                  value,
              });
              continue;
          }
          let LETTERS = /[a-z]/i;
          if (char && LETTERS.test(char)) {
              let value = '';
              while (char && LETTERS.test(char)) {
                  if (current === input.length) {
                      break;
                  }
                  value += char;
                  char = input[++current];
              }
              if (value == 'true' || value == 'false' || value === 'null') {
                  tokens.push({
                      type: 'name',
                      value,
                  });
              }
              else {
                  // unknown token, e.g. `nul` which isn't quite `null`
                  current++;
                  continue;
              }
              continue;
          }
          current++;
      }
      return tokens;
  }, strip = (tokens) => {
      if (tokens.length === 0) {
          return tokens;
      }
      let lastToken = tokens[tokens.length - 1];
      switch (lastToken.type) {
          case 'separator':
              tokens = tokens.slice(0, tokens.length - 1);
              return strip(tokens);
          case 'number':
              let lastCharacterOfLastToken = lastToken.value[lastToken.value.length - 1];
              if (lastCharacterOfLastToken === '.' || lastCharacterOfLastToken === '-') {
                  tokens = tokens.slice(0, tokens.length - 1);
                  return strip(tokens);
              }
          case 'string':
              let tokenBeforeTheLastToken = tokens[tokens.length - 2];
              if (tokenBeforeTheLastToken?.type === 'delimiter') {
                  tokens = tokens.slice(0, tokens.length - 1);
                  return strip(tokens);
              }
              else if (tokenBeforeTheLastToken?.type === 'brace' && tokenBeforeTheLastToken.value === '{') {
                  tokens = tokens.slice(0, tokens.length - 1);
                  return strip(tokens);
              }
              break;
          case 'delimiter':
              tokens = tokens.slice(0, tokens.length - 1);
              return strip(tokens);
      }
      return tokens;
  }, unstrip = (tokens) => {
      let tail = [];
      tokens.map((token) => {
          if (token.type === 'brace') {
              if (token.value === '{') {
                  tail.push('}');
              }
              else {
                  tail.splice(tail.lastIndexOf('}'), 1);
              }
          }
          if (token.type === 'paren') {
              if (token.value === '[') {
                  tail.push(']');
              }
              else {
                  tail.splice(tail.lastIndexOf(']'), 1);
              }
          }
      });
      if (tail.length > 0) {
          tail.reverse().map((item) => {
              if (item === '}') {
                  tokens.push({
                      type: 'brace',
                      value: '}',
                  });
              }
              else if (item === ']') {
                  tokens.push({
                      type: 'paren',
                      value: ']',
                  });
              }
          });
      }
      return tokens;
  }, generate = (tokens) => {
      let output = '';
      tokens.map((token) => {
          switch (token.type) {
              case 'string':
                  output += '"' + token.value + '"';
                  break;
              default:
                  output += token.value;
                  break;
          }
      });
      return output;
  }, partialParse = (input) => JSON.parse(generate(unstrip(strip(tokenize(input)))));

  var __classPrivateFieldSet$1 = (undefined && undefined.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
      if (kind === "m") throw new TypeError("Private method is not writable");
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
      return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
  };
  var __classPrivateFieldGet$1 = (undefined && undefined.__classPrivateFieldGet) || function (receiver, state, kind, f) {
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
      return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
  };
  var _BetaMessageStream_instances, _BetaMessageStream_currentMessageSnapshot, _BetaMessageStream_connectedPromise, _BetaMessageStream_resolveConnectedPromise, _BetaMessageStream_rejectConnectedPromise, _BetaMessageStream_endPromise, _BetaMessageStream_resolveEndPromise, _BetaMessageStream_rejectEndPromise, _BetaMessageStream_listeners, _BetaMessageStream_ended, _BetaMessageStream_errored, _BetaMessageStream_aborted, _BetaMessageStream_catchingPromiseCreated, _BetaMessageStream_response, _BetaMessageStream_request_id, _BetaMessageStream_getFinalMessage, _BetaMessageStream_getFinalText, _BetaMessageStream_handleError, _BetaMessageStream_beginRequest, _BetaMessageStream_addStreamEvent, _BetaMessageStream_endRequest, _BetaMessageStream_accumulateMessage;
  const JSON_BUF_PROPERTY$1 = '__json_buf';
  class BetaMessageStream {
      constructor() {
          _BetaMessageStream_instances.add(this);
          this.messages = [];
          this.receivedMessages = [];
          _BetaMessageStream_currentMessageSnapshot.set(this, void 0);
          this.controller = new AbortController();
          _BetaMessageStream_connectedPromise.set(this, void 0);
          _BetaMessageStream_resolveConnectedPromise.set(this, () => { });
          _BetaMessageStream_rejectConnectedPromise.set(this, () => { });
          _BetaMessageStream_endPromise.set(this, void 0);
          _BetaMessageStream_resolveEndPromise.set(this, () => { });
          _BetaMessageStream_rejectEndPromise.set(this, () => { });
          _BetaMessageStream_listeners.set(this, {});
          _BetaMessageStream_ended.set(this, false);
          _BetaMessageStream_errored.set(this, false);
          _BetaMessageStream_aborted.set(this, false);
          _BetaMessageStream_catchingPromiseCreated.set(this, false);
          _BetaMessageStream_response.set(this, void 0);
          _BetaMessageStream_request_id.set(this, void 0);
          _BetaMessageStream_handleError.set(this, (error) => {
              __classPrivateFieldSet$1(this, _BetaMessageStream_errored, true, "f");
              if (error instanceof Error && error.name === 'AbortError') {
                  error = new APIUserAbortError();
              }
              if (error instanceof APIUserAbortError) {
                  __classPrivateFieldSet$1(this, _BetaMessageStream_aborted, true, "f");
                  return this._emit('abort', error);
              }
              if (error instanceof AnthropicError) {
                  return this._emit('error', error);
              }
              if (error instanceof Error) {
                  const anthropicError = new AnthropicError(error.message);
                  // @ts-ignore
                  anthropicError.cause = error;
                  return this._emit('error', anthropicError);
              }
              return this._emit('error', new AnthropicError(String(error)));
          });
          __classPrivateFieldSet$1(this, _BetaMessageStream_connectedPromise, new Promise((resolve, reject) => {
              __classPrivateFieldSet$1(this, _BetaMessageStream_resolveConnectedPromise, resolve, "f");
              __classPrivateFieldSet$1(this, _BetaMessageStream_rejectConnectedPromise, reject, "f");
          }), "f");
          __classPrivateFieldSet$1(this, _BetaMessageStream_endPromise, new Promise((resolve, reject) => {
              __classPrivateFieldSet$1(this, _BetaMessageStream_resolveEndPromise, resolve, "f");
              __classPrivateFieldSet$1(this, _BetaMessageStream_rejectEndPromise, reject, "f");
          }), "f");
          // Don't let these promises cause unhandled rejection errors.
          // we will manually cause an unhandled rejection error later
          // if the user hasn't registered any error listener or called
          // any promise-returning method.
          __classPrivateFieldGet$1(this, _BetaMessageStream_connectedPromise, "f").catch(() => { });
          __classPrivateFieldGet$1(this, _BetaMessageStream_endPromise, "f").catch(() => { });
      }
      get response() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_response, "f");
      }
      get request_id() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_request_id, "f");
      }
      /**
       * Returns the `MessageStream` data, the raw `Response` instance and the ID of the request,
       * returned vie the `request-id` header which is useful for debugging requests and resporting
       * issues to Anthropic.
       *
       * This is the same as the `APIPromise.withResponse()` method.
       *
       * This method will raise an error if you created the stream using `MessageStream.fromReadableStream`
       * as no `Response` is available.
       */
      async withResponse() {
          const response = await __classPrivateFieldGet$1(this, _BetaMessageStream_connectedPromise, "f");
          if (!response) {
              throw new Error('Could not resolve a `Response` object');
          }
          return {
              data: this,
              response,
              request_id: response.headers.get('request-id'),
          };
      }
      /**
       * Intended for use on the frontend, consuming a stream produced with
       * `.toReadableStream()` on the backend.
       *
       * Note that messages sent to the model do not appear in `.on('message')`
       * in this context.
       */
      static fromReadableStream(stream) {
          const runner = new BetaMessageStream();
          runner._run(() => runner._fromReadableStream(stream));
          return runner;
      }
      static createMessage(messages, params, options) {
          const runner = new BetaMessageStream();
          for (const message of params.messages) {
              runner._addMessageParam(message);
          }
          runner._run(() => runner._createMessage(messages, { ...params, stream: true }, { ...options, headers: { ...options?.headers, 'X-Stainless-Helper-Method': 'stream' } }));
          return runner;
      }
      _run(executor) {
          executor().then(() => {
              this._emitFinal();
              this._emit('end');
          }, __classPrivateFieldGet$1(this, _BetaMessageStream_handleError, "f"));
      }
      _addMessageParam(message) {
          this.messages.push(message);
      }
      _addMessage(message, emit = true) {
          this.receivedMessages.push(message);
          if (emit) {
              this._emit('message', message);
          }
      }
      async _createMessage(messages, params, options) {
          const signal = options?.signal;
          if (signal) {
              if (signal.aborted)
                  this.controller.abort();
              signal.addEventListener('abort', () => this.controller.abort());
          }
          __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_beginRequest).call(this);
          const { response, data: stream } = await messages
              .create({ ...params, stream: true }, { ...options, signal: this.controller.signal })
              .withResponse();
          this._connected(response);
          for await (const event of stream) {
              __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_addStreamEvent).call(this, event);
          }
          if (stream.controller.signal?.aborted) {
              throw new APIUserAbortError();
          }
          __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_endRequest).call(this);
      }
      _connected(response) {
          if (this.ended)
              return;
          __classPrivateFieldSet$1(this, _BetaMessageStream_response, response, "f");
          __classPrivateFieldSet$1(this, _BetaMessageStream_request_id, response?.headers.get('request-id'), "f");
          __classPrivateFieldGet$1(this, _BetaMessageStream_resolveConnectedPromise, "f").call(this, response);
          this._emit('connect');
      }
      get ended() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_ended, "f");
      }
      get errored() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_errored, "f");
      }
      get aborted() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_aborted, "f");
      }
      abort() {
          this.controller.abort();
      }
      /**
       * Adds the listener function to the end of the listeners array for the event.
       * No checks are made to see if the listener has already been added. Multiple calls passing
       * the same combination of event and listener will result in the listener being added, and
       * called, multiple times.
       * @returns this MessageStream, so that calls can be chained
       */
      on(event, listener) {
          const listeners = __classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event] || (__classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event] = []);
          listeners.push({ listener });
          return this;
      }
      /**
       * Removes the specified listener from the listener array for the event.
       * off() will remove, at most, one instance of a listener from the listener array. If any single
       * listener has been added multiple times to the listener array for the specified event, then
       * off() must be called multiple times to remove each instance.
       * @returns this MessageStream, so that calls can be chained
       */
      off(event, listener) {
          const listeners = __classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event];
          if (!listeners)
              return this;
          const index = listeners.findIndex((l) => l.listener === listener);
          if (index >= 0)
              listeners.splice(index, 1);
          return this;
      }
      /**
       * Adds a one-time listener function for the event. The next time the event is triggered,
       * this listener is removed and then invoked.
       * @returns this MessageStream, so that calls can be chained
       */
      once(event, listener) {
          const listeners = __classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event] || (__classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event] = []);
          listeners.push({ listener, once: true });
          return this;
      }
      /**
       * This is similar to `.once()`, but returns a Promise that resolves the next time
       * the event is triggered, instead of calling a listener callback.
       * @returns a Promise that resolves the next time given event is triggered,
       * or rejects if an error is emitted.  (If you request the 'error' event,
       * returns a promise that resolves with the error).
       *
       * Example:
       *
       *   const message = await stream.emitted('message') // rejects if the stream errors
       */
      emitted(event) {
          return new Promise((resolve, reject) => {
              __classPrivateFieldSet$1(this, _BetaMessageStream_catchingPromiseCreated, true, "f");
              if (event !== 'error')
                  this.once('error', reject);
              this.once(event, resolve);
          });
      }
      async done() {
          __classPrivateFieldSet$1(this, _BetaMessageStream_catchingPromiseCreated, true, "f");
          await __classPrivateFieldGet$1(this, _BetaMessageStream_endPromise, "f");
      }
      get currentMessage() {
          return __classPrivateFieldGet$1(this, _BetaMessageStream_currentMessageSnapshot, "f");
      }
      /**
       * @returns a promise that resolves with the the final assistant Message response,
       * or rejects if an error occurred or the stream ended prematurely without producing a Message.
       */
      async finalMessage() {
          await this.done();
          return __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_getFinalMessage).call(this);
      }
      /**
       * @returns a promise that resolves with the the final assistant Message's text response, concatenated
       * together if there are more than one text blocks.
       * Rejects if an error occurred or the stream ended prematurely without producing a Message.
       */
      async finalText() {
          await this.done();
          return __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_getFinalText).call(this);
      }
      _emit(event, ...args) {
          // make sure we don't emit any MessageStreamEvents after end
          if (__classPrivateFieldGet$1(this, _BetaMessageStream_ended, "f"))
              return;
          if (event === 'end') {
              __classPrivateFieldSet$1(this, _BetaMessageStream_ended, true, "f");
              __classPrivateFieldGet$1(this, _BetaMessageStream_resolveEndPromise, "f").call(this);
          }
          const listeners = __classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event];
          if (listeners) {
              __classPrivateFieldGet$1(this, _BetaMessageStream_listeners, "f")[event] = listeners.filter((l) => !l.once);
              listeners.forEach(({ listener }) => listener(...args));
          }
          if (event === 'abort') {
              const error = args[0];
              if (!__classPrivateFieldGet$1(this, _BetaMessageStream_catchingPromiseCreated, "f") && !listeners?.length) {
                  Promise.reject(error);
              }
              __classPrivateFieldGet$1(this, _BetaMessageStream_rejectConnectedPromise, "f").call(this, error);
              __classPrivateFieldGet$1(this, _BetaMessageStream_rejectEndPromise, "f").call(this, error);
              this._emit('end');
              return;
          }
          if (event === 'error') {
              // NOTE: _emit('error', error) should only be called from #handleError().
              const error = args[0];
              if (!__classPrivateFieldGet$1(this, _BetaMessageStream_catchingPromiseCreated, "f") && !listeners?.length) {
                  // Trigger an unhandled rejection if the user hasn't registered any error handlers.
                  // If you are seeing stack traces here, make sure to handle errors via either:
                  // - runner.on('error', () => ...)
                  // - await runner.done()
                  // - await runner.final...()
                  // - etc.
                  Promise.reject(error);
              }
              __classPrivateFieldGet$1(this, _BetaMessageStream_rejectConnectedPromise, "f").call(this, error);
              __classPrivateFieldGet$1(this, _BetaMessageStream_rejectEndPromise, "f").call(this, error);
              this._emit('end');
          }
      }
      _emitFinal() {
          const finalMessage = this.receivedMessages.at(-1);
          if (finalMessage) {
              this._emit('finalMessage', __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_getFinalMessage).call(this));
          }
      }
      async _fromReadableStream(readableStream, options) {
          const signal = options?.signal;
          if (signal) {
              if (signal.aborted)
                  this.controller.abort();
              signal.addEventListener('abort', () => this.controller.abort());
          }
          __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_beginRequest).call(this);
          this._connected(null);
          const stream = Stream.fromReadableStream(readableStream, this.controller);
          for await (const event of stream) {
              __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_addStreamEvent).call(this, event);
          }
          if (stream.controller.signal?.aborted) {
              throw new APIUserAbortError();
          }
          __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_endRequest).call(this);
      }
      [(_BetaMessageStream_currentMessageSnapshot = new WeakMap(), _BetaMessageStream_connectedPromise = new WeakMap(), _BetaMessageStream_resolveConnectedPromise = new WeakMap(), _BetaMessageStream_rejectConnectedPromise = new WeakMap(), _BetaMessageStream_endPromise = new WeakMap(), _BetaMessageStream_resolveEndPromise = new WeakMap(), _BetaMessageStream_rejectEndPromise = new WeakMap(), _BetaMessageStream_listeners = new WeakMap(), _BetaMessageStream_ended = new WeakMap(), _BetaMessageStream_errored = new WeakMap(), _BetaMessageStream_aborted = new WeakMap(), _BetaMessageStream_catchingPromiseCreated = new WeakMap(), _BetaMessageStream_response = new WeakMap(), _BetaMessageStream_request_id = new WeakMap(), _BetaMessageStream_handleError = new WeakMap(), _BetaMessageStream_instances = new WeakSet(), _BetaMessageStream_getFinalMessage = function _BetaMessageStream_getFinalMessage() {
          if (this.receivedMessages.length === 0) {
              throw new AnthropicError('stream ended without producing a Message with role=assistant');
          }
          return this.receivedMessages.at(-1);
      }, _BetaMessageStream_getFinalText = function _BetaMessageStream_getFinalText() {
          if (this.receivedMessages.length === 0) {
              throw new AnthropicError('stream ended without producing a Message with role=assistant');
          }
          const textBlocks = this.receivedMessages
              .at(-1)
              .content.filter((block) => block.type === 'text')
              .map((block) => block.text);
          if (textBlocks.length === 0) {
              throw new AnthropicError('stream ended without producing a content block with type=text');
          }
          return textBlocks.join(' ');
      }, _BetaMessageStream_beginRequest = function _BetaMessageStream_beginRequest() {
          if (this.ended)
              return;
          __classPrivateFieldSet$1(this, _BetaMessageStream_currentMessageSnapshot, undefined, "f");
      }, _BetaMessageStream_addStreamEvent = function _BetaMessageStream_addStreamEvent(event) {
          if (this.ended)
              return;
          const messageSnapshot = __classPrivateFieldGet$1(this, _BetaMessageStream_instances, "m", _BetaMessageStream_accumulateMessage).call(this, event);
          this._emit('streamEvent', event, messageSnapshot);
          switch (event.type) {
              case 'content_block_delta': {
                  const content = messageSnapshot.content.at(-1);
                  switch (event.delta.type) {
                      case 'text_delta': {
                          if (content.type === 'text') {
                              this._emit('text', event.delta.text, content.text || '');
                          }
                          break;
                      }
                      case 'citations_delta': {
                          if (content.type === 'text') {
                              this._emit('citation', event.delta.citation, content.citations ?? []);
                          }
                          break;
                      }
                      case 'input_json_delta': {
                          if (content.type === 'tool_use' && content.input) {
                              this._emit('inputJson', event.delta.partial_json, content.input);
                          }
                          break;
                      }
                      default:
                          checkNever$1(event.delta);
                  }
                  break;
              }
              case 'message_stop': {
                  this._addMessageParam(messageSnapshot);
                  this._addMessage(messageSnapshot, true);
                  break;
              }
              case 'content_block_stop': {
                  this._emit('contentBlock', messageSnapshot.content.at(-1));
                  break;
              }
              case 'message_start': {
                  __classPrivateFieldSet$1(this, _BetaMessageStream_currentMessageSnapshot, messageSnapshot, "f");
                  break;
              }
          }
      }, _BetaMessageStream_endRequest = function _BetaMessageStream_endRequest() {
          if (this.ended) {
              throw new AnthropicError(`stream has ended, this shouldn't happen`);
          }
          const snapshot = __classPrivateFieldGet$1(this, _BetaMessageStream_currentMessageSnapshot, "f");
          if (!snapshot) {
              throw new AnthropicError(`request ended without sending any chunks`);
          }
          __classPrivateFieldSet$1(this, _BetaMessageStream_currentMessageSnapshot, undefined, "f");
          return snapshot;
      }, _BetaMessageStream_accumulateMessage = function _BetaMessageStream_accumulateMessage(event) {
          let snapshot = __classPrivateFieldGet$1(this, _BetaMessageStream_currentMessageSnapshot, "f");
          if (event.type === 'message_start') {
              if (snapshot) {
                  throw new AnthropicError(`Unexpected event order, got ${event.type} before receiving "message_stop"`);
              }
              return event.message;
          }
          if (!snapshot) {
              throw new AnthropicError(`Unexpected event order, got ${event.type} before "message_start"`);
          }
          switch (event.type) {
              case 'message_stop':
                  return snapshot;
              case 'message_delta':
                  snapshot.stop_reason = event.delta.stop_reason;
                  snapshot.stop_sequence = event.delta.stop_sequence;
                  snapshot.usage.output_tokens = event.usage.output_tokens;
                  return snapshot;
              case 'content_block_start':
                  snapshot.content.push(event.content_block);
                  return snapshot;
              case 'content_block_delta': {
                  const snapshotContent = snapshot.content.at(event.index);
                  switch (event.delta.type) {
                      case 'text_delta': {
                          if (snapshotContent?.type === 'text') {
                              snapshotContent.text += event.delta.text;
                          }
                          break;
                      }
                      case 'citations_delta': {
                          if (snapshotContent?.type === 'text') {
                              snapshotContent.citations ?? (snapshotContent.citations = []);
                              snapshotContent.citations.push(event.delta.citation);
                          }
                          break;
                      }
                      case 'input_json_delta': {
                          if (snapshotContent?.type === 'tool_use') {
                              // we need to keep track of the raw JSON string as well so that we can
                              // re-parse it for each delta, for now we just store it as an untyped
                              // non-enumerable property on the snapshot
                              let jsonBuf = snapshotContent[JSON_BUF_PROPERTY$1] || '';
                              jsonBuf += event.delta.partial_json;
                              Object.defineProperty(snapshotContent, JSON_BUF_PROPERTY$1, {
                                  value: jsonBuf,
                                  enumerable: false,
                                  writable: true,
                              });
                              if (jsonBuf) {
                                  snapshotContent.input = partialParse(jsonBuf);
                              }
                          }
                          break;
                      }
                      default:
                          checkNever$1(event.delta);
                  }
                  return snapshot;
              }
              case 'content_block_stop':
                  return snapshot;
          }
      }, Symbol.asyncIterator)]() {
          const pushQueue = [];
          const readQueue = [];
          let done = false;
          this.on('streamEvent', (event) => {
              const reader = readQueue.shift();
              if (reader) {
                  reader.resolve(event);
              }
              else {
                  pushQueue.push(event);
              }
          });
          this.on('end', () => {
              done = true;
              for (const reader of readQueue) {
                  reader.resolve(undefined);
              }
              readQueue.length = 0;
          });
          this.on('abort', (err) => {
              done = true;
              for (const reader of readQueue) {
                  reader.reject(err);
              }
              readQueue.length = 0;
          });
          this.on('error', (err) => {
              done = true;
              for (const reader of readQueue) {
                  reader.reject(err);
              }
              readQueue.length = 0;
          });
          return {
              next: async () => {
                  if (!pushQueue.length) {
                      if (done) {
                          return { value: undefined, done: true };
                      }
                      return new Promise((resolve, reject) => readQueue.push({ resolve, reject })).then((chunk) => (chunk ? { value: chunk, done: false } : { value: undefined, done: true }));
                  }
                  const chunk = pushQueue.shift();
                  return { value: chunk, done: false };
              },
              return: async () => {
                  this.abort();
                  return { value: undefined, done: true };
              },
          };
      }
      toReadableStream() {
          const stream = new Stream(this[Symbol.asyncIterator].bind(this), this.controller);
          return stream.toReadableStream();
      }
  }
  // used to ensure exhaustive case matching without throwing a runtime error
  function checkNever$1(x) { }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  const DEPRECATED_MODELS$1 = {
      'claude-1.3': 'November 6th, 2024',
      'claude-1.3-100k': 'November 6th, 2024',
      'claude-instant-1.1': 'November 6th, 2024',
      'claude-instant-1.1-100k': 'November 6th, 2024',
      'claude-instant-1.2': 'November 6th, 2024',
      'claude-3-sonnet-20240229': 'July 21st, 2025',
      'claude-2.1': 'July 21st, 2025',
      'claude-2.0': 'July 21st, 2025',
  };
  let Messages$1 = class Messages extends APIResource {
      constructor() {
          super(...arguments);
          this.batches = new Batches$1(this._client);
      }
      create(params, options) {
          const { betas, ...body } = params;
          if (body.model in DEPRECATED_MODELS$1) {
              console.warn(`The model '${body.model}' is deprecated and will reach end-of-life on ${DEPRECATED_MODELS$1[body.model]}\nPlease migrate to a newer model. Visit https://docs.anthropic.com/en/docs/resources/model-deprecations for more information.`);
          }
          return this._client.post('/v1/messages?beta=true', {
              body,
              timeout: this._client._options.timeout ?? 600000,
              ...options,
              headers: {
                  ...(betas?.toString() != null ? { 'anthropic-beta': betas?.toString() } : undefined),
                  ...options?.headers,
              },
              stream: params.stream ?? false,
          });
      }
      /**
       * Create a Message stream
       */
      stream(body, options) {
          return BetaMessageStream.createMessage(this, body, options);
      }
      /**
       * Count the number of tokens in a Message.
       *
       * The Token Count API can be used to count the number of tokens in a Message,
       * including tools, images, and documents, without creating it.
       */
      countTokens(params, options) {
          const { betas, ...body } = params;
          return this._client.post('/v1/messages/count_tokens?beta=true', {
              body,
              ...options,
              headers: {
                  'anthropic-beta': [...(betas ?? []), 'token-counting-2024-11-01'].toString(),
                  ...options?.headers,
              },
          });
      }
  };
  Messages$1.Batches = Batches$1;
  Messages$1.BetaMessageBatchesPage = BetaMessageBatchesPage;

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Beta extends APIResource {
      constructor() {
          super(...arguments);
          this.models = new Models$1(this._client);
          this.messages = new Messages$1(this._client);
      }
  }
  Beta.Models = Models$1;
  Beta.BetaModelInfosPage = BetaModelInfosPage;
  Beta.Messages = Messages$1;

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Completions extends APIResource {
      create(body, options) {
          return this._client.post('/v1/complete', {
              body,
              timeout: this._client._options.timeout ?? 600000,
              ...options,
              stream: body.stream ?? false,
          });
      }
  }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Batches extends APIResource {
      /**
       * Send a batch of Message creation requests.
       *
       * The Message Batches API can be used to process multiple Messages API requests at
       * once. Once a Message Batch is created, it begins processing immediately. Batches
       * can take up to 24 hours to complete.
       */
      create(body, options) {
          return this._client.post('/v1/messages/batches', { body, ...options });
      }
      /**
       * This endpoint is idempotent and can be used to poll for Message Batch
       * completion. To access the results of a Message Batch, make a request to the
       * `results_url` field in the response.
       */
      retrieve(messageBatchId, options) {
          return this._client.get(`/v1/messages/batches/${messageBatchId}`, options);
      }
      list(query = {}, options) {
          if (isRequestOptions(query)) {
              return this.list({}, query);
          }
          return this._client.getAPIList('/v1/messages/batches', MessageBatchesPage, { query, ...options });
      }
      /**
       * Delete a Message Batch.
       *
       * Message Batches can only be deleted once they've finished processing. If you'd
       * like to delete an in-progress batch, you must first cancel it.
       */
      delete(messageBatchId, options) {
          return this._client.delete(`/v1/messages/batches/${messageBatchId}`, options);
      }
      /**
       * Batches may be canceled any time before processing ends. Once cancellation is
       * initiated, the batch enters a `canceling` state, at which time the system may
       * complete any in-progress, non-interruptible requests before finalizing
       * cancellation.
       *
       * The number of canceled requests is specified in `request_counts`. To determine
       * which requests were canceled, check the individual results within the batch.
       * Note that cancellation may not result in any canceled requests if they were
       * non-interruptible.
       */
      cancel(messageBatchId, options) {
          return this._client.post(`/v1/messages/batches/${messageBatchId}/cancel`, options);
      }
      /**
       * Streams the results of a Message Batch as a `.jsonl` file.
       *
       * Each line in the file is a JSON object containing the result of a single request
       * in the Message Batch. Results are not guaranteed to be in the same order as
       * requests. Use the `custom_id` field to match results to requests.
       */
      async results(messageBatchId, options) {
          const batch = await this.retrieve(messageBatchId);
          if (!batch.results_url) {
              throw new AnthropicError(`No batch \`results_url\`; Has it finished processing? ${batch.processing_status} - ${batch.id}`);
          }
          return this._client
              .get(batch.results_url, {
              ...options,
              headers: {
                  Accept: 'application/binary',
                  ...options?.headers,
              },
              __binaryResponse: true,
          })
              ._thenUnwrap((_, props) => JSONLDecoder.fromResponse(props.response, props.controller));
      }
  }
  class MessageBatchesPage extends Page {
  }
  Batches.MessageBatchesPage = MessageBatchesPage;

  var __classPrivateFieldSet = (undefined && undefined.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
      if (kind === "m") throw new TypeError("Private method is not writable");
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
      return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
  };
  var __classPrivateFieldGet = (undefined && undefined.__classPrivateFieldGet) || function (receiver, state, kind, f) {
      if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
      if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
      return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
  };
  var _MessageStream_instances, _MessageStream_currentMessageSnapshot, _MessageStream_connectedPromise, _MessageStream_resolveConnectedPromise, _MessageStream_rejectConnectedPromise, _MessageStream_endPromise, _MessageStream_resolveEndPromise, _MessageStream_rejectEndPromise, _MessageStream_listeners, _MessageStream_ended, _MessageStream_errored, _MessageStream_aborted, _MessageStream_catchingPromiseCreated, _MessageStream_response, _MessageStream_request_id, _MessageStream_getFinalMessage, _MessageStream_getFinalText, _MessageStream_handleError, _MessageStream_beginRequest, _MessageStream_addStreamEvent, _MessageStream_endRequest, _MessageStream_accumulateMessage;
  const JSON_BUF_PROPERTY = '__json_buf';
  class MessageStream {
      constructor() {
          _MessageStream_instances.add(this);
          this.messages = [];
          this.receivedMessages = [];
          _MessageStream_currentMessageSnapshot.set(this, void 0);
          this.controller = new AbortController();
          _MessageStream_connectedPromise.set(this, void 0);
          _MessageStream_resolveConnectedPromise.set(this, () => { });
          _MessageStream_rejectConnectedPromise.set(this, () => { });
          _MessageStream_endPromise.set(this, void 0);
          _MessageStream_resolveEndPromise.set(this, () => { });
          _MessageStream_rejectEndPromise.set(this, () => { });
          _MessageStream_listeners.set(this, {});
          _MessageStream_ended.set(this, false);
          _MessageStream_errored.set(this, false);
          _MessageStream_aborted.set(this, false);
          _MessageStream_catchingPromiseCreated.set(this, false);
          _MessageStream_response.set(this, void 0);
          _MessageStream_request_id.set(this, void 0);
          _MessageStream_handleError.set(this, (error) => {
              __classPrivateFieldSet(this, _MessageStream_errored, true, "f");
              if (error instanceof Error && error.name === 'AbortError') {
                  error = new APIUserAbortError();
              }
              if (error instanceof APIUserAbortError) {
                  __classPrivateFieldSet(this, _MessageStream_aborted, true, "f");
                  return this._emit('abort', error);
              }
              if (error instanceof AnthropicError) {
                  return this._emit('error', error);
              }
              if (error instanceof Error) {
                  const anthropicError = new AnthropicError(error.message);
                  // @ts-ignore
                  anthropicError.cause = error;
                  return this._emit('error', anthropicError);
              }
              return this._emit('error', new AnthropicError(String(error)));
          });
          __classPrivateFieldSet(this, _MessageStream_connectedPromise, new Promise((resolve, reject) => {
              __classPrivateFieldSet(this, _MessageStream_resolveConnectedPromise, resolve, "f");
              __classPrivateFieldSet(this, _MessageStream_rejectConnectedPromise, reject, "f");
          }), "f");
          __classPrivateFieldSet(this, _MessageStream_endPromise, new Promise((resolve, reject) => {
              __classPrivateFieldSet(this, _MessageStream_resolveEndPromise, resolve, "f");
              __classPrivateFieldSet(this, _MessageStream_rejectEndPromise, reject, "f");
          }), "f");
          // Don't let these promises cause unhandled rejection errors.
          // we will manually cause an unhandled rejection error later
          // if the user hasn't registered any error listener or called
          // any promise-returning method.
          __classPrivateFieldGet(this, _MessageStream_connectedPromise, "f").catch(() => { });
          __classPrivateFieldGet(this, _MessageStream_endPromise, "f").catch(() => { });
      }
      get response() {
          return __classPrivateFieldGet(this, _MessageStream_response, "f");
      }
      get request_id() {
          return __classPrivateFieldGet(this, _MessageStream_request_id, "f");
      }
      /**
       * Returns the `MessageStream` data, the raw `Response` instance and the ID of the request,
       * returned vie the `request-id` header which is useful for debugging requests and resporting
       * issues to Anthropic.
       *
       * This is the same as the `APIPromise.withResponse()` method.
       *
       * This method will raise an error if you created the stream using `MessageStream.fromReadableStream`
       * as no `Response` is available.
       */
      async withResponse() {
          const response = await __classPrivateFieldGet(this, _MessageStream_connectedPromise, "f");
          if (!response) {
              throw new Error('Could not resolve a `Response` object');
          }
          return {
              data: this,
              response,
              request_id: response.headers.get('request-id'),
          };
      }
      /**
       * Intended for use on the frontend, consuming a stream produced with
       * `.toReadableStream()` on the backend.
       *
       * Note that messages sent to the model do not appear in `.on('message')`
       * in this context.
       */
      static fromReadableStream(stream) {
          const runner = new MessageStream();
          runner._run(() => runner._fromReadableStream(stream));
          return runner;
      }
      static createMessage(messages, params, options) {
          const runner = new MessageStream();
          for (const message of params.messages) {
              runner._addMessageParam(message);
          }
          runner._run(() => runner._createMessage(messages, { ...params, stream: true }, { ...options, headers: { ...options?.headers, 'X-Stainless-Helper-Method': 'stream' } }));
          return runner;
      }
      _run(executor) {
          executor().then(() => {
              this._emitFinal();
              this._emit('end');
          }, __classPrivateFieldGet(this, _MessageStream_handleError, "f"));
      }
      _addMessageParam(message) {
          this.messages.push(message);
      }
      _addMessage(message, emit = true) {
          this.receivedMessages.push(message);
          if (emit) {
              this._emit('message', message);
          }
      }
      async _createMessage(messages, params, options) {
          const signal = options?.signal;
          if (signal) {
              if (signal.aborted)
                  this.controller.abort();
              signal.addEventListener('abort', () => this.controller.abort());
          }
          __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_beginRequest).call(this);
          const { response, data: stream } = await messages
              .create({ ...params, stream: true }, { ...options, signal: this.controller.signal })
              .withResponse();
          this._connected(response);
          for await (const event of stream) {
              __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_addStreamEvent).call(this, event);
          }
          if (stream.controller.signal?.aborted) {
              throw new APIUserAbortError();
          }
          __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_endRequest).call(this);
      }
      _connected(response) {
          if (this.ended)
              return;
          __classPrivateFieldSet(this, _MessageStream_response, response, "f");
          __classPrivateFieldSet(this, _MessageStream_request_id, response?.headers.get('request-id'), "f");
          __classPrivateFieldGet(this, _MessageStream_resolveConnectedPromise, "f").call(this, response);
          this._emit('connect');
      }
      get ended() {
          return __classPrivateFieldGet(this, _MessageStream_ended, "f");
      }
      get errored() {
          return __classPrivateFieldGet(this, _MessageStream_errored, "f");
      }
      get aborted() {
          return __classPrivateFieldGet(this, _MessageStream_aborted, "f");
      }
      abort() {
          this.controller.abort();
      }
      /**
       * Adds the listener function to the end of the listeners array for the event.
       * No checks are made to see if the listener has already been added. Multiple calls passing
       * the same combination of event and listener will result in the listener being added, and
       * called, multiple times.
       * @returns this MessageStream, so that calls can be chained
       */
      on(event, listener) {
          const listeners = __classPrivateFieldGet(this, _MessageStream_listeners, "f")[event] || (__classPrivateFieldGet(this, _MessageStream_listeners, "f")[event] = []);
          listeners.push({ listener });
          return this;
      }
      /**
       * Removes the specified listener from the listener array for the event.
       * off() will remove, at most, one instance of a listener from the listener array. If any single
       * listener has been added multiple times to the listener array for the specified event, then
       * off() must be called multiple times to remove each instance.
       * @returns this MessageStream, so that calls can be chained
       */
      off(event, listener) {
          const listeners = __classPrivateFieldGet(this, _MessageStream_listeners, "f")[event];
          if (!listeners)
              return this;
          const index = listeners.findIndex((l) => l.listener === listener);
          if (index >= 0)
              listeners.splice(index, 1);
          return this;
      }
      /**
       * Adds a one-time listener function for the event. The next time the event is triggered,
       * this listener is removed and then invoked.
       * @returns this MessageStream, so that calls can be chained
       */
      once(event, listener) {
          const listeners = __classPrivateFieldGet(this, _MessageStream_listeners, "f")[event] || (__classPrivateFieldGet(this, _MessageStream_listeners, "f")[event] = []);
          listeners.push({ listener, once: true });
          return this;
      }
      /**
       * This is similar to `.once()`, but returns a Promise that resolves the next time
       * the event is triggered, instead of calling a listener callback.
       * @returns a Promise that resolves the next time given event is triggered,
       * or rejects if an error is emitted.  (If you request the 'error' event,
       * returns a promise that resolves with the error).
       *
       * Example:
       *
       *   const message = await stream.emitted('message') // rejects if the stream errors
       */
      emitted(event) {
          return new Promise((resolve, reject) => {
              __classPrivateFieldSet(this, _MessageStream_catchingPromiseCreated, true, "f");
              if (event !== 'error')
                  this.once('error', reject);
              this.once(event, resolve);
          });
      }
      async done() {
          __classPrivateFieldSet(this, _MessageStream_catchingPromiseCreated, true, "f");
          await __classPrivateFieldGet(this, _MessageStream_endPromise, "f");
      }
      get currentMessage() {
          return __classPrivateFieldGet(this, _MessageStream_currentMessageSnapshot, "f");
      }
      /**
       * @returns a promise that resolves with the the final assistant Message response,
       * or rejects if an error occurred or the stream ended prematurely without producing a Message.
       */
      async finalMessage() {
          await this.done();
          return __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_getFinalMessage).call(this);
      }
      /**
       * @returns a promise that resolves with the the final assistant Message's text response, concatenated
       * together if there are more than one text blocks.
       * Rejects if an error occurred or the stream ended prematurely without producing a Message.
       */
      async finalText() {
          await this.done();
          return __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_getFinalText).call(this);
      }
      _emit(event, ...args) {
          // make sure we don't emit any MessageStreamEvents after end
          if (__classPrivateFieldGet(this, _MessageStream_ended, "f"))
              return;
          if (event === 'end') {
              __classPrivateFieldSet(this, _MessageStream_ended, true, "f");
              __classPrivateFieldGet(this, _MessageStream_resolveEndPromise, "f").call(this);
          }
          const listeners = __classPrivateFieldGet(this, _MessageStream_listeners, "f")[event];
          if (listeners) {
              __classPrivateFieldGet(this, _MessageStream_listeners, "f")[event] = listeners.filter((l) => !l.once);
              listeners.forEach(({ listener }) => listener(...args));
          }
          if (event === 'abort') {
              const error = args[0];
              if (!__classPrivateFieldGet(this, _MessageStream_catchingPromiseCreated, "f") && !listeners?.length) {
                  Promise.reject(error);
              }
              __classPrivateFieldGet(this, _MessageStream_rejectConnectedPromise, "f").call(this, error);
              __classPrivateFieldGet(this, _MessageStream_rejectEndPromise, "f").call(this, error);
              this._emit('end');
              return;
          }
          if (event === 'error') {
              // NOTE: _emit('error', error) should only be called from #handleError().
              const error = args[0];
              if (!__classPrivateFieldGet(this, _MessageStream_catchingPromiseCreated, "f") && !listeners?.length) {
                  // Trigger an unhandled rejection if the user hasn't registered any error handlers.
                  // If you are seeing stack traces here, make sure to handle errors via either:
                  // - runner.on('error', () => ...)
                  // - await runner.done()
                  // - await runner.final...()
                  // - etc.
                  Promise.reject(error);
              }
              __classPrivateFieldGet(this, _MessageStream_rejectConnectedPromise, "f").call(this, error);
              __classPrivateFieldGet(this, _MessageStream_rejectEndPromise, "f").call(this, error);
              this._emit('end');
          }
      }
      _emitFinal() {
          const finalMessage = this.receivedMessages.at(-1);
          if (finalMessage) {
              this._emit('finalMessage', __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_getFinalMessage).call(this));
          }
      }
      async _fromReadableStream(readableStream, options) {
          const signal = options?.signal;
          if (signal) {
              if (signal.aborted)
                  this.controller.abort();
              signal.addEventListener('abort', () => this.controller.abort());
          }
          __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_beginRequest).call(this);
          this._connected(null);
          const stream = Stream.fromReadableStream(readableStream, this.controller);
          for await (const event of stream) {
              __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_addStreamEvent).call(this, event);
          }
          if (stream.controller.signal?.aborted) {
              throw new APIUserAbortError();
          }
          __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_endRequest).call(this);
      }
      [(_MessageStream_currentMessageSnapshot = new WeakMap(), _MessageStream_connectedPromise = new WeakMap(), _MessageStream_resolveConnectedPromise = new WeakMap(), _MessageStream_rejectConnectedPromise = new WeakMap(), _MessageStream_endPromise = new WeakMap(), _MessageStream_resolveEndPromise = new WeakMap(), _MessageStream_rejectEndPromise = new WeakMap(), _MessageStream_listeners = new WeakMap(), _MessageStream_ended = new WeakMap(), _MessageStream_errored = new WeakMap(), _MessageStream_aborted = new WeakMap(), _MessageStream_catchingPromiseCreated = new WeakMap(), _MessageStream_response = new WeakMap(), _MessageStream_request_id = new WeakMap(), _MessageStream_handleError = new WeakMap(), _MessageStream_instances = new WeakSet(), _MessageStream_getFinalMessage = function _MessageStream_getFinalMessage() {
          if (this.receivedMessages.length === 0) {
              throw new AnthropicError('stream ended without producing a Message with role=assistant');
          }
          return this.receivedMessages.at(-1);
      }, _MessageStream_getFinalText = function _MessageStream_getFinalText() {
          if (this.receivedMessages.length === 0) {
              throw new AnthropicError('stream ended without producing a Message with role=assistant');
          }
          const textBlocks = this.receivedMessages
              .at(-1)
              .content.filter((block) => block.type === 'text')
              .map((block) => block.text);
          if (textBlocks.length === 0) {
              throw new AnthropicError('stream ended without producing a content block with type=text');
          }
          return textBlocks.join(' ');
      }, _MessageStream_beginRequest = function _MessageStream_beginRequest() {
          if (this.ended)
              return;
          __classPrivateFieldSet(this, _MessageStream_currentMessageSnapshot, undefined, "f");
      }, _MessageStream_addStreamEvent = function _MessageStream_addStreamEvent(event) {
          if (this.ended)
              return;
          const messageSnapshot = __classPrivateFieldGet(this, _MessageStream_instances, "m", _MessageStream_accumulateMessage).call(this, event);
          this._emit('streamEvent', event, messageSnapshot);
          switch (event.type) {
              case 'content_block_delta': {
                  const content = messageSnapshot.content.at(-1);
                  switch (event.delta.type) {
                      case 'text_delta': {
                          if (content.type === 'text') {
                              this._emit('text', event.delta.text, content.text || '');
                          }
                          break;
                      }
                      case 'citations_delta': {
                          if (content.type === 'text') {
                              this._emit('citation', event.delta.citation, content.citations ?? []);
                          }
                          break;
                      }
                      case 'input_json_delta': {
                          if (content.type === 'tool_use' && content.input) {
                              this._emit('inputJson', event.delta.partial_json, content.input);
                          }
                          break;
                      }
                      default:
                          checkNever(event.delta);
                  }
                  break;
              }
              case 'message_stop': {
                  this._addMessageParam(messageSnapshot);
                  this._addMessage(messageSnapshot, true);
                  break;
              }
              case 'content_block_stop': {
                  this._emit('contentBlock', messageSnapshot.content.at(-1));
                  break;
              }
              case 'message_start': {
                  __classPrivateFieldSet(this, _MessageStream_currentMessageSnapshot, messageSnapshot, "f");
                  break;
              }
          }
      }, _MessageStream_endRequest = function _MessageStream_endRequest() {
          if (this.ended) {
              throw new AnthropicError(`stream has ended, this shouldn't happen`);
          }
          const snapshot = __classPrivateFieldGet(this, _MessageStream_currentMessageSnapshot, "f");
          if (!snapshot) {
              throw new AnthropicError(`request ended without sending any chunks`);
          }
          __classPrivateFieldSet(this, _MessageStream_currentMessageSnapshot, undefined, "f");
          return snapshot;
      }, _MessageStream_accumulateMessage = function _MessageStream_accumulateMessage(event) {
          let snapshot = __classPrivateFieldGet(this, _MessageStream_currentMessageSnapshot, "f");
          if (event.type === 'message_start') {
              if (snapshot) {
                  throw new AnthropicError(`Unexpected event order, got ${event.type} before receiving "message_stop"`);
              }
              return event.message;
          }
          if (!snapshot) {
              throw new AnthropicError(`Unexpected event order, got ${event.type} before "message_start"`);
          }
          switch (event.type) {
              case 'message_stop':
                  return snapshot;
              case 'message_delta':
                  snapshot.stop_reason = event.delta.stop_reason;
                  snapshot.stop_sequence = event.delta.stop_sequence;
                  snapshot.usage.output_tokens = event.usage.output_tokens;
                  return snapshot;
              case 'content_block_start':
                  snapshot.content.push(event.content_block);
                  return snapshot;
              case 'content_block_delta': {
                  const snapshotContent = snapshot.content.at(event.index);
                  switch (event.delta.type) {
                      case 'text_delta': {
                          if (snapshotContent?.type === 'text') {
                              snapshotContent.text += event.delta.text;
                          }
                          break;
                      }
                      case 'citations_delta': {
                          if (snapshotContent?.type === 'text') {
                              snapshotContent.citations ?? (snapshotContent.citations = []);
                              snapshotContent.citations.push(event.delta.citation);
                          }
                          break;
                      }
                      case 'input_json_delta': {
                          if (snapshotContent?.type === 'tool_use') {
                              // we need to keep track of the raw JSON string as well so that we can
                              // re-parse it for each delta, for now we just store it as an untyped
                              // non-enumerable property on the snapshot
                              let jsonBuf = snapshotContent[JSON_BUF_PROPERTY] || '';
                              jsonBuf += event.delta.partial_json;
                              Object.defineProperty(snapshotContent, JSON_BUF_PROPERTY, {
                                  value: jsonBuf,
                                  enumerable: false,
                                  writable: true,
                              });
                              if (jsonBuf) {
                                  snapshotContent.input = partialParse(jsonBuf);
                              }
                          }
                          break;
                      }
                      default:
                          checkNever(event.delta);
                  }
                  return snapshot;
              }
              case 'content_block_stop':
                  return snapshot;
          }
      }, Symbol.asyncIterator)]() {
          const pushQueue = [];
          const readQueue = [];
          let done = false;
          this.on('streamEvent', (event) => {
              const reader = readQueue.shift();
              if (reader) {
                  reader.resolve(event);
              }
              else {
                  pushQueue.push(event);
              }
          });
          this.on('end', () => {
              done = true;
              for (const reader of readQueue) {
                  reader.resolve(undefined);
              }
              readQueue.length = 0;
          });
          this.on('abort', (err) => {
              done = true;
              for (const reader of readQueue) {
                  reader.reject(err);
              }
              readQueue.length = 0;
          });
          this.on('error', (err) => {
              done = true;
              for (const reader of readQueue) {
                  reader.reject(err);
              }
              readQueue.length = 0;
          });
          return {
              next: async () => {
                  if (!pushQueue.length) {
                      if (done) {
                          return { value: undefined, done: true };
                      }
                      return new Promise((resolve, reject) => readQueue.push({ resolve, reject })).then((chunk) => (chunk ? { value: chunk, done: false } : { value: undefined, done: true }));
                  }
                  const chunk = pushQueue.shift();
                  return { value: chunk, done: false };
              },
              return: async () => {
                  this.abort();
                  return { value: undefined, done: true };
              },
          };
      }
      toReadableStream() {
          const stream = new Stream(this[Symbol.asyncIterator].bind(this), this.controller);
          return stream.toReadableStream();
      }
  }
  // used to ensure exhaustive case matching without throwing a runtime error
  function checkNever(x) { }

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Messages extends APIResource {
      constructor() {
          super(...arguments);
          this.batches = new Batches(this._client);
      }
      create(body, options) {
          if (body.model in DEPRECATED_MODELS) {
              console.warn(`The model '${body.model}' is deprecated and will reach end-of-life on ${DEPRECATED_MODELS[body.model]}\nPlease migrate to a newer model. Visit https://docs.anthropic.com/en/docs/resources/model-deprecations for more information.`);
          }
          return this._client.post('/v1/messages', {
              body,
              timeout: this._client._options.timeout ?? 600000,
              ...options,
              stream: body.stream ?? false,
          });
      }
      /**
       * Create a Message stream
       */
      stream(body, options) {
          return MessageStream.createMessage(this, body, options);
      }
      /**
       * Count the number of tokens in a Message.
       *
       * The Token Count API can be used to count the number of tokens in a Message,
       * including tools, images, and documents, without creating it.
       */
      countTokens(body, options) {
          return this._client.post('/v1/messages/count_tokens', { body, ...options });
      }
  }
  const DEPRECATED_MODELS = {
      'claude-1.3': 'November 6th, 2024',
      'claude-1.3-100k': 'November 6th, 2024',
      'claude-instant-1.1': 'November 6th, 2024',
      'claude-instant-1.1-100k': 'November 6th, 2024',
      'claude-instant-1.2': 'November 6th, 2024',
      'claude-3-sonnet-20240229': 'July 21st, 2025',
      'claude-2.1': 'July 21st, 2025',
      'claude-2.0': 'July 21st, 2025',
  };
  Messages.Batches = Batches;
  Messages.MessageBatchesPage = MessageBatchesPage;

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  class Models extends APIResource {
      /**
       * Get a specific model.
       *
       * The Models API response can be used to determine information about a specific
       * model or resolve a model alias to a model ID.
       */
      retrieve(modelId, options) {
          return this._client.get(`/v1/models/${modelId}`, options);
      }
      list(query = {}, options) {
          if (isRequestOptions(query)) {
              return this.list({}, query);
          }
          return this._client.getAPIList('/v1/models', ModelInfosPage, { query, ...options });
      }
  }
  class ModelInfosPage extends Page {
  }
  Models.ModelInfosPage = ModelInfosPage;

  // File generated from our OpenAPI spec by Stainless. See CONTRIBUTING.md for details.
  var _a;
  /**
   * API Client for interfacing with the Anthropic API.
   */
  class Anthropic extends APIClient {
      /**
       * API Client for interfacing with the Anthropic API.
       *
       * @param {string | null | undefined} [opts.apiKey=process.env['ANTHROPIC_API_KEY'] ?? null]
       * @param {string | null | undefined} [opts.authToken=process.env['ANTHROPIC_AUTH_TOKEN'] ?? null]
       * @param {string} [opts.baseURL=process.env['ANTHROPIC_BASE_URL'] ?? https://api.anthropic.com] - Override the default base URL for the API.
       * @param {number} [opts.timeout=10 minutes] - The maximum amount of time (in milliseconds) the client will wait for a response before timing out.
       * @param {number} [opts.httpAgent] - An HTTP agent used to manage HTTP(s) connections.
       * @param {Core.Fetch} [opts.fetch] - Specify a custom `fetch` function implementation.
       * @param {number} [opts.maxRetries=2] - The maximum number of times the client will retry a request.
       * @param {Core.Headers} opts.defaultHeaders - Default headers to include with every request to the API.
       * @param {Core.DefaultQuery} opts.defaultQuery - Default query parameters to include with every request to the API.
       * @param {boolean} [opts.dangerouslyAllowBrowser=false] - By default, client-side use of this library is not allowed, as it risks exposing your secret API credentials to attackers.
       */
      constructor({ baseURL = readEnv('ANTHROPIC_BASE_URL'), apiKey = readEnv('ANTHROPIC_API_KEY') ?? null, authToken = readEnv('ANTHROPIC_AUTH_TOKEN') ?? null, ...opts } = {}) {
          const options = {
              apiKey,
              authToken,
              ...opts,
              baseURL: baseURL || `https://api.anthropic.com`,
          };
          if (!options.dangerouslyAllowBrowser && isRunningInBrowser()) {
              throw new AnthropicError("It looks like you're running in a browser-like environment.\n\nThis is disabled by default, as it risks exposing your secret API credentials to attackers.\nIf you understand the risks and have appropriate mitigations in place,\nyou can set the `dangerouslyAllowBrowser` option to `true`, e.g.,\n\nnew Anthropic({ apiKey, dangerouslyAllowBrowser: true });\n");
          }
          super({
              baseURL: options.baseURL,
              timeout: options.timeout ?? 600000 /* 10 minutes */,
              httpAgent: options.httpAgent,
              maxRetries: options.maxRetries,
              fetch: options.fetch,
          });
          this.completions = new Completions(this);
          this.messages = new Messages(this);
          this.models = new Models(this);
          this.beta = new Beta(this);
          this._options = options;
          this.apiKey = apiKey;
          this.authToken = authToken;
      }
      defaultQuery() {
          return this._options.defaultQuery;
      }
      defaultHeaders(opts) {
          return {
              ...super.defaultHeaders(opts),
              ...(this._options.dangerouslyAllowBrowser ?
                  { 'anthropic-dangerous-direct-browser-access': 'true' }
                  : undefined),
              'anthropic-version': '2023-06-01',
              ...this._options.defaultHeaders,
          };
      }
      validateHeaders(headers, customHeaders) {
          if (this.apiKey && headers['x-api-key']) {
              return;
          }
          if (customHeaders['x-api-key'] === null) {
              return;
          }
          if (this.authToken && headers['authorization']) {
              return;
          }
          if (customHeaders['authorization'] === null) {
              return;
          }
          throw new Error('Could not resolve authentication method. Expected either apiKey or authToken to be set. Or for one of the "X-Api-Key" or "Authorization" headers to be explicitly omitted');
      }
      authHeaders(opts) {
          const apiKeyAuth = this.apiKeyAuth(opts);
          const bearerAuth = this.bearerAuth(opts);
          if (apiKeyAuth != null && !isEmptyObj(apiKeyAuth)) {
              return apiKeyAuth;
          }
          if (bearerAuth != null && !isEmptyObj(bearerAuth)) {
              return bearerAuth;
          }
          return {};
      }
      apiKeyAuth(opts) {
          if (this.apiKey == null) {
              return {};
          }
          return { 'X-Api-Key': this.apiKey };
      }
      bearerAuth(opts) {
          if (this.authToken == null) {
              return {};
          }
          return { Authorization: `Bearer ${this.authToken}` };
      }
  }
  _a = Anthropic;
  Anthropic.Anthropic = _a;
  Anthropic.HUMAN_PROMPT = '\n\nHuman:';
  Anthropic.AI_PROMPT = '\n\nAssistant:';
  Anthropic.DEFAULT_TIMEOUT = 600000; // 10 minutes
  Anthropic.AnthropicError = AnthropicError;
  Anthropic.APIError = APIError;
  Anthropic.APIConnectionError = APIConnectionError;
  Anthropic.APIConnectionTimeoutError = APIConnectionTimeoutError;
  Anthropic.APIUserAbortError = APIUserAbortError;
  Anthropic.NotFoundError = NotFoundError;
  Anthropic.ConflictError = ConflictError;
  Anthropic.RateLimitError = RateLimitError;
  Anthropic.BadRequestError = BadRequestError;
  Anthropic.AuthenticationError = AuthenticationError;
  Anthropic.InternalServerError = InternalServerError;
  Anthropic.PermissionDeniedError = PermissionDeniedError;
  Anthropic.UnprocessableEntityError = UnprocessableEntityError;
  Anthropic.toFile = toFile;
  Anthropic.fileFromPath = fileFromPath;
  Anthropic.Completions = Completions;
  Anthropic.Messages = Messages;
  Anthropic.Models = Models;
  Anthropic.ModelInfosPage = ModelInfosPage;
  Anthropic.Beta = Beta;
  const { HUMAN_PROMPT, AI_PROMPT } = Anthropic;

  const BASE_SYSTEM_PROMPT = `You are a helpful terminal assistant integrated into Termin.AI, an interactive terminal with AI overlay.

Your role is to help users with terminal-related tasks, including:
- Understanding and explaining command output
- Suggesting commands to accomplish tasks
- Reading and analyzing files in the current directory
- Debugging issues based on terminal history
- Answering questions about the terminal environment

## Available Tools

You have access to the following tools:

### Built-in Tools (from Claude Agent SDK)
- **Read**: Read file contents from the filesystem
- **Grep**: Search for patterns in files using regex
- **Bash**: Execute shell commands (requires user approval for dangerous commands)
- **Edit**: Edit files with precise string replacements
- **Write**: Write new files or overwrite existing ones
- **Glob**: Find files matching glob patterns

### Termin.AI Custom Tools
- **suggest_command**: Suggest a shell command for the user to execute
  - Use this when you want to propose a command but let the user decide whether to run it
  - The command will be shown in the terminal with an approval prompt
  - For safe commands, auto-approval may be enabled

- **read_scrollback**: Read recent terminal output history
  - Use this to see what commands were run and their output
  - Helpful for debugging issues or understanding context

## Guidelines

1. **Be concise**: Terminal users appreciate brief, actionable responses
2. **Safety first**: Never suggest destructive commands without clear warnings
3. **Context-aware**: Use terminal history and current directory when available
4. **Explain commands**: When suggesting commands, explain what they do
5. **Ask when uncertain**: If you need more information, ask the user

## Command Safety

When suggesting commands, consider:
- **Safe**: ls, pwd, cat, grep, echo, cd, etc.
- **Moderate risk**: git operations, package installs, file edits
- **Dangerous**: rm -rf, sudo operations, system modifications

Always explain the risks for moderate and dangerous commands.`;
  function buildSystemPrompt(context) {
    if (!context) {
      return BASE_SYSTEM_PROMPT;
    }
    const parts = [BASE_SYSTEM_PROMPT];
    parts.push("\n\n## Current Terminal Context\n");
    if (context.osInfo) {
      parts.push(`**Operating System**: ${context.osInfo}`);
    }
    if (context.shell) {
      parts.push(`**Shell**: ${context.shell}`);
    }
    if (context.cwd) {
      parts.push(`**Current Directory**: \`${context.cwd}\``);
    }
    if (context.lastExitCode !== void 0) {
      const status = context.lastExitCode === 0 ? "✓ Success" : `✗ Failed (exit code ${context.lastExitCode})`;
      parts.push(`**Last Command Status**: ${status}`);
    }
    if (context.historyLines && context.historyLines.length > 0) {
      const lineCount = context.historyLines.length;
      const historyText = context.historyLines.map((line, i) => `${i + 1}. ${line}`).join("\n");
      parts.push(
        `
**Recent Terminal History** (last ${lineCount} lines):
\`\`\`
${historyText}
\`\`\``
      );
    }
    return parts.join("\n");
  }

  async function suggestCommand(args) {
    try {
      const result = await globalThis.Deno.core.ops.op_suggest_command(args);
      return result;
    } catch (error) {
      throw new Error(`Error suggesting command: ${error.message}`);
    }
  }
  async function readScrollback(args) {
    try {
      const result = await globalThis.Deno.core.ops.op_read_scrollback(args);
      return result;
    } catch (error) {
      throw new Error(`Error reading scrollback: ${error.message}`);
    }
  }
  function getToolDefinitions() {
    return [
      {
        name: "suggest_command",
        description: "Suggest a shell command to execute in the terminal. The command will be shown to the user for approval before execution.",
        input_schema: {
          type: "object",
          properties: {
            command: {
              type: "string",
              description: "The shell command to suggest (e.g., 'ls -la', 'git status')"
            },
            explanation: {
              type: "string",
              description: "Optional explanation of what the command does and why it's being suggested"
            }
          },
          required: ["command"]
        }
      },
      {
        name: "read_scrollback",
        description: "Read the terminal scrollback history to see recent command output and terminal activity.",
        input_schema: {
          type: "object",
          properties: {
            numLines: {
              type: "number",
              description: "Number of lines to read from the scrollback buffer (default: 100, max: 1000)"
            }
          }
        }
      }
    ];
  }
  async function executeTool(toolName, input) {
    try {
      switch (toolName) {
        case "suggest_command": {
          const args = input;
          const result = await suggestCommand(args);
          return { content: result, isError: false };
        }
        case "read_scrollback": {
          const args = input;
          const result = await readScrollback(args);
          return { content: result, isError: false };
        }
        default:
          return {
            content: `Unknown tool: ${toolName}`,
            isError: true
          };
      }
    } catch (error) {
      return {
        content: error.message,
        isError: true
      };
    }
  }

  const DEFAULT_MAX_TOKENS = 4096;
  const MAX_TOOL_LOOPS = 10;
  async function chatStream(options) {
    const messages = [];
    const startTime = Date.now();
    let totalUsage = {
      inputTokens: 0,
      outputTokens: 0
    };
    try {
      const apiKey = options.apiKey;
      if (!apiKey) {
        throw new Error("API key is required for Anthropic provider");
      }
      const client = new Anthropic({
        apiKey
      });
      const systemPrompt = options.systemPrompt ?? buildSystemPrompt(options.terminalContext);
      const conversationMessages = [
        {
          role: "user",
          content: options.message
        }
      ];
      const tools = getToolDefinitions();
      let loopCount = 0;
      while (loopCount < MAX_TOOL_LOOPS) {
        loopCount++;
        const response = await client.messages.create({
          model: options.model,
          max_tokens: DEFAULT_MAX_TOKENS,
          system: systemPrompt,
          messages: conversationMessages,
          tools
        });
        totalUsage.inputTokens += response.usage.input_tokens;
        totalUsage.outputTokens += response.usage.output_tokens;
        let hasToolUse = false;
        const toolUseBlocks = [];
        for (const block of response.content) {
          if (block.type === "text") {
            if (block.text) {
              messages.push({
                type: "text",
                content: block.text
              });
            }
          } else if (block.type === "tool_use") {
            hasToolUse = true;
            toolUseBlocks.push(block);
            messages.push({
              type: "tool_call",
              toolName: block.name,
              toolInput: block.input
            });
          }
        }
        if (hasToolUse) {
          conversationMessages.push({
            role: "assistant",
            content: response.content
          });
          const toolResults = [];
          for (const toolUse of toolUseBlocks) {
            const result = await executeTool(toolUse.name, toolUse.input);
            toolResults.push({
              type: "tool_result",
              tool_use_id: toolUse.id,
              content: result.content,
              is_error: result.isError
            });
          }
          conversationMessages.push({
            role: "user",
            content: toolResults
          });
          continue;
        }
        break;
      }
      const durationMs = Date.now() - startTime;
      messages.push({
        type: "result",
        isError: false,
        result: "Chat completed successfully",
        usage: totalUsage,
        durationMs
      });
    } catch (error) {
      const durationMs = Date.now() - startTime;
      messages.push({
        type: "result",
        isError: true,
        errors: [error.message],
        usage: totalUsage,
        durationMs
      });
    }
    return messages;
  }
  async function testAgent() {
    return "Agent module loaded successfully with Anthropic SDK. Tools available: suggest_command, read_scrollback";
  }
  function echo(message) {
    return `Echo: ${message}`;
  }
  function add(a, b) {
    return a + b;
  }

  function initializeAgent() {
    const terminai = {
      // Main agent functions
      chatStream,
      testAgent,
      // Test/interop functions
      echo,
      add,
      // Tool functions (can be called directly for testing)
      suggestCommand,
      readScrollback,
      executeTool,
      // Utility functions
      buildSystemPrompt,
      getToolDefinitions,
      // Version info
      version: "1.0.0",
      // Check if module is loaded
      isLoaded: true
    };
    globalThis.terminai = terminai;
    globalThis.chatStream = chatStream;
    globalThis.testAgent = testAgent;
    globalThis.echo = echo;
    globalThis.add = add;
  }
  initializeAgent();
  try {
    if (typeof globalThis.Deno?.core?.print === "function") {
      globalThis.Deno.core.print("[terminai-agent] Module initialized with Anthropic SDK\n");
    }
  } catch {
  }

  exports.add = add;
  exports.buildSystemPrompt = buildSystemPrompt;
  exports.chatStream = chatStream;
  exports.echo = echo;
  exports.executeTool = executeTool;
  exports.getToolDefinitions = getToolDefinitions;
  exports.readScrollback = readScrollback;
  exports.suggestCommand = suggestCommand;
  exports.testAgent = testAgent;

  Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });

})(this.terminaiAgent = this.terminaiAgent || {});
//# sourceMappingURL=agent.js.map
