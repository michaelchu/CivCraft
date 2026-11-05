"""Regenerate tax vectors with the unmodified Freeciv C function.

Usage: python3 tests/fixtures/freeciv_normal_tax_oracle.py --reference /path/to/freeciv
Requires clang. No Freeciv server is built or run. City-derived budget inputs
are injected through dai_calc_data; Normal H_RATES is enabled and the bundled
Civ2Civ3 Rapture_Grow/Tech_Parasite bonuses are absent. All rate-selection and
Hamilton-distribution code is extracted verbatim from the reference source.
"""
import argparse
import hashlib
import random
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--reference", type=Path, required=True)
parser.add_argument("--output", type=Path, default=Path(__file__).with_name("freeciv_normal_tax_vectors.tsv"))
args = parser.parse_args()
reference = args.reference

src=(reference / "ai/default/daihand.c").read_text()
a=src.index('static void dai_manage_taxes(');start=src.index('{',a);depth=1;i=start+1
while depth:
 depth+=(src[i]=='{')-(src[i]=='}');i+=1
function=src[a:i]
dist=(reference / "utility/distribute.c").read_text();a=dist.index('void distribute(');dist=dist[a:]
stubs=r'''
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <assert.h>
#define MIN(a,b) ((a)<(b)?(a):(b))
#define MAX(a,b) ((a)>(b)?(a):(b))
#define CLIP(a,b,c) MIN(MAX(a,b),c)
#define FC_INFINITY 1000000000
#define fc_assert(x) assert(x)
#define fc_assert_ret(x) assert(x)
#define log_base(...)
#define CITY_LOG(...)
#define LOGLEVEL_TAX 0
#define LOG_ERROR 0
#define AI_GOLD_RESERVE_MIN_TURNS 10
#define AI_BULBS_RESERVE_MIN_TURNS 10
#define PCT_DELTA_TAX 50
#define PCT_DELTA_SCI 10
#define TECH_UPKEEP_NONE 0
#define H_RATES 1
#define H_AWAY 2
#define EFT_MAX_RATES 1
#define EFT_RAPTURE_GROW 2
#define O_FOOD 0
#define O_GOLD 3
#define RATE_NOT_SET -1
#define RATE_VALID(x) ((x)!=-1)
#define RATE_REMAINS(r) MAX(0,100-(r)[0]-(r)[1]-(r)[2])
enum {AI_RATE_SCI,AI_RATE_TAX,AI_RATE_LUX,AI_RATE_COUNT};
enum celebration {AI_CELEBRATION_UNCHECKED,AI_CELEBRATION_NO,AI_CELEBRATION_YES};
struct civ_map {int dummy;}; struct {struct civ_map map;} wld;
struct {struct {bool changable_tax;int rapturedelay,tech_upkeep_style,celebratesize;} info;int government_during_revolution;} game;
struct city {int surplus[6];};struct ai_city {bool celebrate;};
struct player {struct {unsigned science,tax,luxury;int gold;} economic;struct {int bulbs_last_turn;} server;void *cities,*connections;};
struct research {int bulbs_researched;};struct adv_data {bool celebrate;};
struct cm_parameter {bool require_happy,allow_disorder,allow_specialists;int factor[6],minimal_surplus[6];};
struct cm_result {bool found_a_valid;};struct ai_type {int dummy;};
int cap,trade_total,expense_total,income_total,gold_reserve,tech_upkeep,wants,war;
struct research research;struct adv_data adv;struct ai_city ai_city;
#define city_list_iterate(list,name) for(struct city *name=NULL;name;name=NULL)
#define city_list_iterate_end
bool has_handicap(struct player*p,int id){return id==H_RATES;}
int get_player_bonus(struct player*p,int id){return id==EFT_MAX_RATES?cap:0;}
struct research *research_get(struct player*p){return &research;}
struct adv_data *adv_data_get(struct player*p,void*v){return &adv;}
int government_of_player(struct player*p){return 0;}
void cm_init_parameter(struct cm_parameter*p){memset(p,0,sizeof(*p));}
void dai_calc_data(struct player*p,int*t,int*e,int*i){*t=trade_total;*e=expense_total;*i=income_total;}
int dai_gold_reserve(struct player*p){return gold_reserve;}
int player_tech_upkeep(struct player*p){return tech_upkeep;}
bool adv_wants_science(struct player*p){return wants;}
bool dai_on_war_footing(struct ai_type*a,struct player*p){return war;}
struct cm_result *cm_result_new(struct city*c){return calloc(1,sizeof(struct cm_result));}
struct ai_city *def_ai_city_data(struct city*c,struct ai_type*a){return &ai_city;}
void cm_clear_cache(struct city*c){}
void cm_query_result(struct city*c,struct cm_parameter*p,struct cm_result*r,bool n){}
int city_size_get(struct city*c){return 0;}
bool city_can_grow_to(struct city*c,int size){return false;}
void cm_result_destroy(struct cm_result*c){free(c);}
void apply_cmresult_to_city(struct city*c,struct cm_result*r){}
void city_refresh_from_main_map(const struct civ_map*m,struct city*c,void*v){}
bool city_happy(struct city*c){return false;}
void send_player_info_c(struct player*p,void*v){}
'''
main=r'''
int main(int argc,char**argv){
 if(argc!=15)return 3;int x[14];for(int i=0;i<14;i++)x[i]=atoi(argv[i+1]);
 cap=x[0];trade_total=x[1];expense_total=x[2];income_total=x[3];gold_reserve=x[5];tech_upkeep=x[6];wants=x[12];war=x[13];
 struct player p={0};p.economic.gold=x[4];research.bulbs_researched=x[7];p.server.bulbs_last_turn=x[8];
 p.economic.science=x[9];p.economic.tax=x[10];p.economic.luxury=x[11];
 game.info.changable_tax=true;game.info.rapturedelay=1;game.info.tech_upkeep_style=1;game.info.celebratesize=3;game.government_during_revolution=-1;
 dai_manage_taxes(NULL,&p);printf("%u,%u,%u\n",p.economic.science,p.economic.tax,p.economic.luxury);
}
'''
with tempfile.TemporaryDirectory(prefix="civcraft-tax-oracle-") as temporary:
    directory = Path(temporary)
    c_file = directory / "oracle.c"
    binary = directory / "oracle"
    c_file.write_text(stubs + dist + "\n" + function + "\n" + main)
    subprocess.run(["clang", "-O2", "-Dnullptr=NULL", "-DTRUE=true", "-DFALSE=false",
                    str(c_file), "-o", str(binary)], check=True)
    random_source = random.Random(344)
    rows = []
    for _ in range(64):
        cap = random_source.choice([60, 70, 80, 90])
        saved = random_source.choice([(s, t, 100-s-t) for s in range(0, cap+1, 10)
                                     for t in range(0, cap+1, 10) if 0 <= 100-s-t <= cap])
        values = [cap, random_source.choice([0, 1, 7, 20, 100, 300]),
                  random_source.choice([0, 3, 20, 90, 250]),
                  random_source.choice([0, 3, 40, 100, 200]),
                  random_source.choice([0, 3, 16, 100, 1000]),
                  random_source.choice([2, 20, 100, 1000]),
                  random_source.choice([0, 1, 10, 70, 120]),
                  random_source.choice([0, 15, 16, 160, 1600]),
                  random_source.choice([-100, 0, 50, 200]), *saved,
                  random_source.randrange(2), random_source.randrange(2)]
        result = subprocess.check_output([str(binary), *map(str, values)], text=True).strip()
        rows.append("\t".join(map(str, values)) + "\t" + result.replace(",", "\t"))
    digest = hashlib.sha256((reference / "ai/default/daihand.c").read_bytes()).hexdigest()
    header = ("# Literal dai_manage_taxes + utility/distribute.c compiled with clang -O2; "
              "Normal H_RATES, Civ2Civ3 no Rapture_Grow/Tech_Parasite.\n"
              f"# daihand.c SHA256 {digest}\n"
              "# maxrate trade expenses grossGold gold reserve upkeep bulbs lastScience "
              "savedScience savedTax savedLuxury wantsScience warFooting => science tax luxury\n")
    args.output.write_text(header + "\n".join(rows) + "\n")

