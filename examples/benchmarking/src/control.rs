#[cfg(test)]
mod benches {
    use factorio_rs::prelude::*;

    #[factorio_rs::bench(iterations = 1000)]
    unsafe fn dense_no_prealloc() {
        lua! {
            local x = {}
            for i = 1, 100000 do
                x[i] = i
            end
        }
    }

    #[factorio_rs::bench(iterations = 1000)]
    unsafe fn dense_prealloc() {
        lua! {
            local x = {}
            x[100000] = false
            x[100000] = nil

            for i = 1, 100000 do
                x[i] = i
            end
        }
    }

    #[factorio_rs::bench(iterations = 1000)]
    unsafe fn sparse_no_prealloc() {
        lua! {
            local x = {}
            for i = 1, 100000 do
                x[i * 2] = i
            end
        }
    }

    #[factorio_rs::bench(iterations = 1000)]
    unsafe fn sparse_prealloc() {
        lua! {
            local x = {}
            x[200000] = false
            x[200000] = nil

            for i = 1, 100000 do
                x[i * 2] = i
            end
        }
    }
}

#[factorio_rs::event(OnSingleplayerInit)]
pub fn on_singleplayer_init() {}
