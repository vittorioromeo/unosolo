#/bin/bash

RED='\033[0;31m'
NC='\033[0m'

mkdir ./tempunosolotest
cd ./tempunosolotest

git clone https://github.com/SuperV1234/scelta.git && \
unosolo -p"./scelta/include" -t"./scelta/include/scelta.hpp" > scelta_sh.hpp && \
g++ -std=c++1z ./scelta_sh.hpp && \
clang++ -std=c++1z ./scelta_sh.hpp && \
echo -e "${RED}scelta OK!${NC}"

git clone https://github.com/SuperV1234/vrm_pp.git && \
unosolo -p"./vrm_pp/include" -t"./vrm_pp/include/vrm/pp.hpp" > vrm_pp_sh.hpp && \
g++ -std=c++1z ./vrm_pp_sh.hpp && \
clang++ -std=c++1z ./vrm_pp_sh.hpp && \
echo -e "${RED}vrm_pp OK!${NC}"

git clone https://github.com/SuperV1234/vrm_core.git && \
unosolo -p"./vrm_pp/include" "./vrm_core/include" -t"./vrm_core/include/vrm/core.hpp" > vrm_core_sh.hpp && \
g++ -std=c++1z ./vrm_core_sh.hpp && \
clang++ -std=c++1z ./vrm_core_sh.hpp && \
echo -e "${RED}vrm_core OK!${NC}"

git clone https://github.com/boostorg/hana && \
unosolo -p"./hana/include" -t"./hana/include/boost/hana.hpp" > hana_sh.hpp && \
g++ -std=c++1z ./hana_sh.hpp && \
clang++ -std=c++1z ./hana_sh.hpp && \
echo -e "${RED}hana OK!${NC}"
