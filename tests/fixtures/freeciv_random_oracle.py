#!/usr/bin/env python3
"""Extract and compile reference utility/rand.c; regenerate gameplay RNG vectors.
Usage: python3 tests/fixtures/freeciv_random_oracle.py /path/to/freeciv [output.tsv]
Only logging/assert infrastructure is stubbed; RNG function bodies are verbatim.
"""
import pathlib
import hashlib
import subprocess
import sys
import tempfile

PINNED_REVISION = '9ff184b6eb128ca636c7ad6269e27aa959600006'
reference = pathlib.Path(sys.argv[1])
revision = subprocess.check_output(
    ['git', '-C', str(reference), 'rev-parse', 'HEAD'], text=True
).strip()
if revision != PINNED_REVISION:
    raise SystemExit(f'expected Freeciv {PINNED_REVISION}, found {revision}')
source_path = reference / 'utility/rand.c'
source_bytes = source_path.read_bytes()
source_hash = hashlib.sha256(source_bytes).hexdigest()
source = source_bytes.decode()
def extract(signature):
    start = source.index(signature)
    brace = source.index('{', start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]

harness = r'''
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#define MAX_UINT32 UINT32_MAX
#define TRUE true
#define log_error(...) ((void)0)
#define RAND_VALUE_LOG(...) ((void)0)
#define fc_assert_ret_val(condition,value) do { if (!(condition)) return value; } while (0)
typedef uint32_t RANDOM_TYPE;
typedef struct { RANDOM_TYPE v[56]; int j,k,x; bool is_init; } RANDOM_STATE;
static RANDOM_STATE rand_state;
#define fc_rand(size) fc_rand_debug(size,"oracle",0,"")
'''
harness += extract('RANDOM_TYPE fc_rand_debug(') + '\n' + extract('void fc_srand(')
harness += r'''
static uint32_t state_hash(void) {
  uint32_t hash=2166136261u;
  for(int i=0;i<56;i++) hash=(hash ^ rand_state.v[i])*16777619u;
  hash=(hash ^ rand_state.j)*16777619u;
  hash=(hash ^ rand_state.k)*16777619u;
  hash=(hash ^ rand_state.x)*16777619u;
  return (hash ^ rand_state.is_init)*16777619u;
}
int main(void) {
  uint32_t seeds[]={0,1,42,2147483647u,4294967295u};
  uint32_t sizes[]={0,1,2,3,7,56,100,1000,2147483648u,2147483649u,4294967294u,4294967295u};
  puts("# seed\tsize\tvalue\tstate_hash (literal utility/rand.c, clang -O2)");
  for(unsigned seed=0;seed<5;seed++) {
    fc_srand(seeds[seed]);
    for(int draw=0;draw<144;draw++) {
      uint32_t size=sizes[draw%12],value=fc_rand(size);
      printf("%u\t%u\t%u\t%u\n",seeds[seed],size,value,state_hash());
    }
  }
}
'''
with tempfile.TemporaryDirectory() as directory:
    src = pathlib.Path(directory) / 'oracle.c'
    binary = pathlib.Path(directory) / 'oracle'
    src.write_text(harness)
    subprocess.run(['clang','-O2',str(src),'-o',str(binary)],check=True)
    result = subprocess.check_output([str(binary)])
output = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).with_name('freeciv_random_vectors.tsv')
provenance = (
    f'# Freeciv source revision: {revision}\n'
    f'# utility/rand.c SHA-256: {source_hash}\n'
).encode()
output.write_bytes(provenance + result)
