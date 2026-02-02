if you don't have rust, paste this in your terminal: 
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
      
once you have rust, run this
      cargo build --release --workspace
            ./target/release/slowdesktop
