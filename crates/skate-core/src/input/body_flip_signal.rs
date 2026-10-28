#[derive(Clone,Copy,Debug)]
pub struct Settings {
    pub gesture_window:f32,
    pub takeoff_window:f32,
}

#[derive(Clone,Copy,Debug,Default)]
pub struct State {
    gesture_time:f32,
    takeoff_time:f32,
    selected:Option<usize>,
}

impl State {
    pub fn begin(&mut self,settings:Settings) {
        self.gesture_time=settings.gesture_window;
        self.takeoff_time=settings.takeoff_window;
    }

    pub fn update(&mut self,present:[bool;2],filtered_category:u32,dt:f32,settings:Settings)->Option<usize> {
        if let Some(index)=present.iter().position(|&value|value) {
            self.gesture_time=0.0;
            self.selected=Some(index);
        } else {
            self.gesture_time=cap(self.gesture_time+dt,settings.gesture_window);
        }
        let gesture=self.gesture_time<settings.gesture_window;
        self.takeoff_time=if filtered_category==2 {
            cap(self.takeoff_time+dt,settings.takeoff_window)
        } else {0.0};
        let takeoff=!(self.takeoff_time<=0.0)&&self.takeoff_time<settings.takeoff_window;
        if gesture&&takeoff {self.selected} else {None}
    }
}

fn cap(value:f32,limit:f32)->f32 {if value-limit>=0.0 {limit} else {value}}
