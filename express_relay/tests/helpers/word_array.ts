export function convertWordArrayToBufferOld(wordArray: any) {
  var len = wordArray.words.length,
    u8_array = new Uint8Array(len << 2),
    offset = 0,
    word,
    i;
  for (i = 0; i < len; i++) {
    word = wordArray.words[i];
    u8_array[offset++] = word >> 24;
    u8_array[offset++] = (word >> 16) & 0xff;
    u8_array[offset++] = (word >> 8) & 0xff;
    u8_array[offset++] = word & 0xff;
  }
  return Buffer.from(u8_array);
}

export function convertWordArrayToBuffer(hash) {
  return (
    hash.words
      //map each word to an array of bytes
      .map(function (v) {
        // create an array of 4 bytes (less if sigBytes says we have run out)
        var bytes = [0, 0, 0, 0]
          .slice(0, Math.min(4, hash.sigBytes))
          // grab that section of the 4 byte word
          .map(function (d, i) {
            return (v >>> (8 * i)) % 256;
          })
          // flip that
          .reverse();
        // remove the bytes we've processed
        // from the bytes we need to process
        hash.sigBytes -= bytes.length;
        return bytes;
      })
      // concatinate all the arrays of bytes
      .reduce(function (a, d) {
        return a.concat(d);
      }, [])
      // convert the 'bytes' to 'characters'
      .map(function (d) {
        return String.fromCharCode(d);
      })
      // create a single block of memory
      .join("")
  );
}

export function wordToByteArray(word, length) {
  var ba = [],
    i,
    xFF = 0xff;
  if (length > 0) ba.push(word >>> 24);
  if (length > 1) ba.push((word >>> 16) & xFF);
  if (length > 2) ba.push((word >>> 8) & xFF);
  if (length > 3) ba.push(word & xFF);

  return ba;
}

export function wordArrayToByteArray(wordArray, length) {
  if (
    wordArray.hasOwnProperty("sigBytes") &&
    wordArray.hasOwnProperty("words")
  ) {
    length = wordArray.sigBytes;
    wordArray = wordArray.words;
  }

  var result = [],
    bytes,
    i = 0;
  while (length > 0) {
    bytes = wordToByteArray(wordArray[i], Math.min(4, length));
    length -= bytes.length;
    result.push(bytes);
    i++;
  }
  return [].concat.apply([], result);
}

export function fromWordArray(wordArray) {
  var bytes = new Uint8Array(wordArray.sigBytes);
  for (var j = 0; j < wordArray.sigBytes; j++) {
    bytes[j] = (wordArray.words[j >>> 2] >>> (24 - 8 * (j % 4))) & 0xff;
  }
  return bytes;
}
