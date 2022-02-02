function theta_deg = lag_to_angle(lag, Ts, d_mics)
    cos_theta = (lag * Ts * 343)/d_mics;
    theta = acos(cos_theta);
    theta_deg = theta*180/pi;
end