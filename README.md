clear cache
cd ~/.Trash 2>/dev/null; cd ~/Downloads 2>/dev/null

cargo clean

to run apps: cd into the directory , 
then 
  cargo build —release 
  , 
then 
  cargo run -p slowApp
