cargo build --release -p cli-indexer
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-COREKS" --set COREKS --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-CORE" --set CORE --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-ALIZE" --set ALIZE --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-BISE" --set BISE --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-CYCLONE" --set CYCLONE --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-DUSTER" --set DUSTER --out ../build/sets_index --profile
..\target\release\cli-indexer.exe build --root "..\..\equinox-cards\cards-unique-EOLE" --set EOLE --out ../build/sets_index --profile
..\target\release\cli-indexer.exe merge --index-dir ../build/sets_index --sets COREKS,CORE,ALIZE,BISE,CYCLONE,DUSTER,EOLE --out ../build/full_index/ALL_SETS
