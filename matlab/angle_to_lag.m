function lag = angle_to_lag(theta_deg, Ts, d_mics)
    theta = theta_deg * pi/180;
    cos_theta = cos(theta);
    lag = (cos_theta*d_mics)/(Ts*343);
end