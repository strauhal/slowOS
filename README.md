clear cache
cd ~/.Trash 2>/dev/null; cd ~/Downloads 2>/dev/null



to run apps: cd into the directory , 
then 

cargo clean && cargo build --release

  , 
then 
  cargo run -p slowApp
