"""Compile literal daitech.c counter-defender function with controlled queries.
Usage: python3 tests/fixtures/freeciv_counter_research_oracle.py /path/to/freeciv
The bundled ruleset has no unit-building prerequisites; its unreachable
building branch is removed. All selection/half-threshold code is verbatim.
"""
from pathlib import Path
import sys, subprocess, tempfile, random
src=(Path(sys.argv[1])/'ai/default/daitech.c').read_text()
a=src.index('struct unit_type *dai_wants_defender_against(');b=src.index('{',a);n=1;i=b+1
while n:
    n+=(src[i]=='{')-(src[i]=='}');i+=1
body=src[a:i]
a=body.index('      building = utype_needs_improvement');b=body.index('      if (cost < best_cost',a)
body=body[:a]+body[b:]
h=r'''
#include <stdio.h>
#include <stdbool.h>
#include <stddef.h>
#define U_LAST 4
#define FALSE false
#define FC_INFINITY 1000000000
#define RPT_CERTAIN 0
#define EFT_DEFEND_BONUS 0
#define CBONUS_DEFENSE_DIVIDER_PCT 0
#define CBONUS_DEFENSE_DIVIDER 0
#define TECH_KNOWN 1
#define A_NEVER NULL
struct advance {int id;};
struct unit_type {int defense_strength,available,later,tech;int bonuses;struct {int defense_mp_bonuses_pct[4],scramble_coeff[4];} cache;};
struct research {int x;}; struct player {int x;}; struct city {int x;};
struct ai_type {int x;}; struct civ_map {int x;}; struct tile {int x;};
struct ai_plr {float tech_want[3];};
static struct unit_type types[4];static struct advance advances[3]={{0},{1},{2}};
static struct research research;static struct ai_plr data;
static int costs[3],reachable[3],selected;
#define research_get(p) (&research)
#define city_tile(c) NULL
#define utype_index(u) ((int)((u)-types))
#define get_unittype_bonus(...) 0
#define combat_bonus_against(...) 0
#define can_city_build_unit_now(n,c,u,r) ((u)->available)
#define can_city_build_unit_later(n,c,u) ((u)->later)
#define research_invention_state(...) 0
#define advance_number(t) ((t)->id)
#define advance_index(t) ((t)->id)
#define research_goal_bulbs_required(r,t) costs[t]
#define research_invention_reachable(r,t) reachable[t]
#define def_ai_player_data(p,a) (&data)
#define TECH_LOG(a,l,p,t,...) (selected=(t)->id)
#define unit_type_iterate(u) for(int ui=0;ui<4;ui++){struct unit_type *u=&types[ui];
#define unit_type_iterate_end }
#define unit_tech_reqs_iterate(u,t) for(int ti=0;ti<1&&(u)->tech>=0;ti++){struct advance*t=&advances[(u)->tech];
#define unit_tech_reqs_iterate_end }
'''
h+=body
h+=r'''
int main(void){int want;while(scanf("%d",&want)==1){
for(int i=0;i<4;i++){
int cost,reach;scanf("%d%d%d%d%d%d",&types[i].defense_strength,&types[i].available,&types[i].later,&types[i].tech,&cost,&reach);
if(types[i].tech>=0){costs[types[i].tech]=cost;reachable[types[i].tech]=reach;}}
for(int i=0;i<3;i++)data.tech_want[i]=0;selected=-1;
dai_wants_defender_against(NULL,NULL,NULL,NULL,types,want);
printf("%d %d\n",selected,selected<0?0:(int)data.tech_want[selected]);}}
'''
r=random.Random(20261105)
rows=[]
# Four input types: baseline, then three future candidates in native order.
for i in range(128):
    want=r.randrange(1,1000)
    units=[(r.randrange(0,6),1,1,-1,0,0)]
    for t in range(3): units.append((r.randrange(0,12),0,r.randrange(2),t,r.choice([40,40,80,160,400]),r.randrange(2)))
    if i<3:
        want=101;units=[(2,1,1,-1,0,0),(3 if i!=1 else 4,0,1,0,40,1),(9,0,1,1,40 if i==2 else 400,1),(1,0,1,2,80,1)]
    rows.append([want]+[v for unit in units for v in unit])
with tempfile.TemporaryDirectory() as d:
    c=Path(d)/'oracle.c';e=Path(d)/'oracle';c.write_text(h)
    subprocess.run(['clang','-O2',str(c),'-o',str(e)],check=True)
    out=subprocess.check_output([str(e)],input=('\n'.join(' '.join(map(str,row)) for row in rows)+'\n').encode()).decode().splitlines()
result='# want; four(defense,available,later,tech,cost,reachable); chosen_tech delta\n'
result+='\n'.join('\t'.join(map(str,row))+ '\t'+line.replace(' ','\t') for row,line in zip(rows,out))+'\n'
Path(__file__).with_name('freeciv_counter_research_vectors.tsv').write_text(result)
